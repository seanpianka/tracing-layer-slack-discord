use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::{StatusCode, Url};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;

use crate::Error;

const MAX_ATTEMPTS: usize = 5;
const DELIVERY_BUDGET: Duration = Duration::from_secs(120);
const INITIAL_BACKOFF: Duration = Duration::from_millis(100);

/// A validated webhook destination whose credentials are always redacted.
#[derive(Clone)]
pub struct WebhookUrl(Url);

impl WebhookUrl {
    pub fn parse(value: impl AsRef<str>) -> Result<Self, Error> {
        let url = Url::parse(value.as_ref()).map_err(|_| Error::InvalidWebhookUrl)?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return Err(Error::InvalidWebhookUrl);
        }
        Ok(Self(url))
    }

    fn as_url(&self) -> &Url {
        &self.0
    }
}

impl fmt::Debug for WebhookUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WebhookUrl(<redacted>)")
    }
}

impl fmt::Display for WebhookUrl {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted webhook URL>")
    }
}

impl std::str::FromStr for WebhookUrl {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

#[derive(Debug)]
enum Command {
    Deliver(String),
    Shutdown,
}

#[derive(Debug)]
struct QueueState {
    accepting: bool,
    sender: mpsc::UnboundedSender<Command>,
}

#[derive(Clone, Debug)]
pub(crate) struct Enqueue(Arc<Mutex<QueueState>>);

impl Enqueue {
    pub(crate) fn send(&self, body: String) -> Result<(), Error> {
        let state = self.0.lock().map_err(|_| Error::DeliveryStopped)?;
        if !state.accepting {
            return Err(Error::DeliveryStopped);
        }
        state
            .sender
            .send(Command::Deliver(body))
            .map_err(|_| Error::DeliveryStopped)
    }

    fn stop(&self) -> Result<(), Error> {
        let mut state = self.0.lock().map_err(|_| Error::DeliveryStopped)?;
        if !state.accepting {
            return Err(Error::DeliveryStopped);
        }
        state.accepting = false;
        state.sender.send(Command::Shutdown).map_err(|_| Error::DeliveryStopped)
    }
}

/// An unstarted webhook delivery worker.
pub struct Delivery {
    webhook_url: WebhookUrl,
    receiver: mpsc::UnboundedReceiver<Command>,
    enqueue: Enqueue,
    adapter: Arc<dyn HttpAdapter>,
    retry: RetrySettings,
}

impl Delivery {
    /// Start delivery on the current Tokio runtime.
    pub fn spawn(self) -> Result<DeliveryHandle, Error> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| Error::RuntimeUnavailable)?;
        let join = runtime.spawn(run_worker(self.webhook_url, self.receiver, self.adapter, self.retry));
        Ok(DeliveryHandle {
            enqueue: self.enqueue,
            join,
        })
    }
}

/// A running webhook delivery worker.
pub struct DeliveryHandle {
    enqueue: Enqueue,
    join: JoinHandle<DeliveryReport>,
}

impl DeliveryHandle {
    /// Stop accepting messages, drain the accepted FIFO, and return its report.
    pub async fn shutdown(self) -> Result<DeliveryReport, Error> {
        let stop_result = self.enqueue.stop();
        let report = self.join.await.map_err(|_| Error::WorkerJoin)?;
        stop_result.map(|_| report)
    }
}

/// A redacted summary of a drained delivery queue.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeliveryReport {
    accepted: usize,
    delivered: usize,
    retries: usize,
    failures: Vec<DeliveryFailure>,
}

impl DeliveryReport {
    pub fn accepted(&self) -> usize {
        self.accepted
    }

    pub fn delivered(&self) -> usize {
        self.delivered
    }

    pub fn retries(&self) -> usize {
        self.retries
    }

    pub fn failures(&self) -> &[DeliveryFailure] {
        &self.failures
    }
}

/// One terminal message-delivery failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeliveryFailure {
    attempts: usize,
    reason: DeliveryFailureReason,
}

impl DeliveryFailure {
    pub fn attempts(&self) -> usize {
        self.attempts
    }

    pub fn reason(&self) -> &DeliveryFailureReason {
        &self.reason
    }
}

/// A redacted terminal reason for failed delivery.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DeliveryFailureReason {
    Network,
    HttpStatus(u16),
    RetryBudgetExceeded,
}

pub(crate) fn delivery_channel(webhook_url: WebhookUrl) -> (Enqueue, Delivery) {
    delivery_channel_with(
        webhook_url,
        Arc::new(ReqwestAdapter(reqwest::Client::new())),
        RetrySettings::default(),
    )
}

fn delivery_channel_with(
    webhook_url: WebhookUrl,
    adapter: Arc<dyn HttpAdapter>,
    retry: RetrySettings,
) -> (Enqueue, Delivery) {
    let (sender, receiver) = mpsc::unbounded_channel();
    let enqueue = Enqueue(Arc::new(Mutex::new(QueueState {
        accepting: true,
        sender,
    })));
    (
        enqueue.clone(),
        Delivery {
            webhook_url,
            receiver,
            enqueue,
            adapter,
            retry,
        },
    )
}

#[derive(Clone, Copy)]
struct RetrySettings {
    max_attempts: usize,
    budget: Duration,
    jitter: bool,
}

impl Default for RetrySettings {
    fn default() -> Self {
        Self {
            max_attempts: MAX_ATTEMPTS,
            budget: DELIVERY_BUDGET,
            jitter: true,
        }
    }
}

#[derive(Clone, Debug)]
enum SendOutcome {
    Response {
        status: StatusCode,
        retry_after: Option<Duration>,
    },
    Network,
}

type SendFuture<'a> = Pin<Box<dyn Future<Output = SendOutcome> + Send + 'a>>;

trait HttpAdapter: Send + Sync {
    fn send<'a>(&'a self, webhook_url: &'a Url, body: &'a str) -> SendFuture<'a>;
}

struct ReqwestAdapter(reqwest::Client);

impl HttpAdapter for ReqwestAdapter {
    fn send<'a>(&'a self, webhook_url: &'a Url, body: &'a str) -> SendFuture<'a> {
        Box::pin(async move {
            match self
                .0
                .post(webhook_url.clone())
                .header("Content-Type", "application/json")
                .body(body.to_owned())
                .send()
                .await
            {
                Ok(response) => SendOutcome::Response {
                    status: response.status(),
                    retry_after: response
                        .headers()
                        .get(reqwest::header::RETRY_AFTER)
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.parse::<f64>().ok())
                        .filter(|seconds| seconds.is_finite() && *seconds >= 0.0)
                        .map(Duration::from_secs_f64),
                },
                Err(_) => SendOutcome::Network,
            }
        })
    }
}

async fn run_worker(
    webhook_url: WebhookUrl,
    mut receiver: mpsc::UnboundedReceiver<Command>,
    adapter: Arc<dyn HttpAdapter>,
    retry: RetrySettings,
) -> DeliveryReport {
    let mut report = DeliveryReport::default();
    while let Some(command) = receiver.recv().await {
        match command {
            Command::Deliver(body) => {
                report.accepted += 1;
                match deliver_one(webhook_url.as_url(), &body, adapter.as_ref(), retry, &mut report).await {
                    Ok(()) => report.delivered += 1,
                    Err(failure) => {
                        #[cfg(feature = "log-errors")]
                        eprintln!("ERROR: webhook delivery failed: {:?}", failure.reason);
                        report.failures.push(failure);
                    }
                }
            }
            Command::Shutdown => break,
        }
    }
    report
}

async fn deliver_one(
    webhook_url: &Url,
    body: &str,
    adapter: &dyn HttpAdapter,
    settings: RetrySettings,
    report: &mut DeliveryReport,
) -> Result<(), DeliveryFailure> {
    let started = Instant::now();
    for attempt in 1..=settings.max_attempts {
        let remaining = settings.budget.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(DeliveryFailure {
                attempts: attempt.saturating_sub(1),
                reason: DeliveryFailureReason::RetryBudgetExceeded,
            });
        }
        let outcome = match tokio::time::timeout(remaining, adapter.send(webhook_url, body)).await {
            Ok(outcome) => outcome,
            Err(_) => {
                return Err(DeliveryFailure {
                    attempts: attempt,
                    reason: DeliveryFailureReason::RetryBudgetExceeded,
                })
            }
        };
        if matches!(&outcome, SendOutcome::Response { status, .. } if status.is_success()) {
            return Ok(());
        }

        let reason = outcome_reason(&outcome);
        if !is_retryable(&outcome) || attempt == settings.max_attempts {
            return Err(DeliveryFailure {
                attempts: attempt,
                reason,
            });
        }

        let delay = match outcome {
            SendOutcome::Response {
                status: StatusCode::TOO_MANY_REQUESTS,
                retry_after: Some(delay),
            } => delay,
            _ => exponential_backoff(attempt, settings.jitter),
        };
        if started.elapsed().saturating_add(delay) >= settings.budget {
            return Err(DeliveryFailure {
                attempts: attempt,
                reason: DeliveryFailureReason::RetryBudgetExceeded,
            });
        }
        tokio::time::sleep(delay).await;
        report.retries += 1;
    }
    unreachable!()
}

fn is_retryable(outcome: &SendOutcome) -> bool {
    match outcome {
        SendOutcome::Network => true,
        SendOutcome::Response { status, .. } => {
            *status == StatusCode::REQUEST_TIMEOUT
                || *status == StatusCode::TOO_MANY_REQUESTS
                || status.is_server_error()
        }
    }
}

fn outcome_reason(outcome: &SendOutcome) -> DeliveryFailureReason {
    match outcome {
        SendOutcome::Network => DeliveryFailureReason::Network,
        SendOutcome::Response { status, .. } => DeliveryFailureReason::HttpStatus(status.as_u16()),
    }
}

fn exponential_backoff(attempt: usize, jitter: bool) -> Duration {
    let base = INITIAL_BACKOFF.saturating_mul(2u32.saturating_pow((attempt - 1) as u32));
    if !jitter {
        return base;
    }
    let upper = (base.as_millis() as u64 / 2).max(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as u64;
    base.saturating_add(Duration::from_millis(nanos % (upper + 1)))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct ScriptedAdapter {
        outcomes: Mutex<VecDeque<SendOutcome>>,
        delays: Mutex<VecDeque<Duration>>,
        bodies: Mutex<Vec<String>>,
        instants: Mutex<Vec<Instant>>,
        calls: AtomicUsize,
    }

    impl ScriptedAdapter {
        fn new(outcomes: impl IntoIterator<Item = SendOutcome>) -> Self {
            Self {
                outcomes: Mutex::new(outcomes.into_iter().collect()),
                delays: Mutex::new(VecDeque::new()),
                bodies: Mutex::new(Vec::new()),
                instants: Mutex::new(Vec::new()),
                calls: AtomicUsize::new(0),
            }
        }

        fn with_delays(self, delays: impl IntoIterator<Item = Duration>) -> Self {
            *self.delays.lock().unwrap() = delays.into_iter().collect();
            self
        }
    }

    impl HttpAdapter for ScriptedAdapter {
        fn send<'a>(&'a self, _webhook_url: &'a Url, body: &'a str) -> SendFuture<'a> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                self.instants.lock().unwrap().push(Instant::now());
                self.bodies.lock().unwrap().push(body.to_owned());
                let delay = self.delays.lock().unwrap().pop_front().unwrap_or_default();
                tokio::time::sleep(delay).await;
                self.outcomes
                    .lock()
                    .unwrap()
                    .pop_front()
                    .unwrap_or(SendOutcome::Response {
                        status: StatusCode::NO_CONTENT,
                        retry_after: None,
                    })
            })
        }
    }

    fn response(status: StatusCode) -> SendOutcome {
        SendOutcome::Response {
            status,
            retry_after: None,
        }
    }

    fn test_delivery(adapter: Arc<dyn HttpAdapter>, budget: Duration) -> (Enqueue, Delivery) {
        delivery_channel_with(
            WebhookUrl::parse("https://example.com/hooks/secret").unwrap(),
            adapter,
            RetrySettings {
                max_attempts: 5,
                budget,
                jitter: false,
            },
        )
    }

    #[test]
    fn webhook_url_is_validated_and_redacted() {
        assert_eq!(WebhookUrl::parse("not a URL").unwrap_err(), Error::InvalidWebhookUrl);
        let url = WebhookUrl::parse("https://example.com/hooks/secret-token").unwrap();
        assert_eq!(format!("{url:?}"), "WebhookUrl(<redacted>)");
        assert!(!url.to_string().contains("secret-token"));
    }

    #[tokio::test]
    async fn reports_success_and_permanent_failure_without_leaking_payloads() {
        let adapter = Arc::new(ScriptedAdapter::new([
            response(StatusCode::NO_CONTENT),
            response(StatusCode::BAD_REQUEST),
        ]));
        let (enqueue, delivery) = test_delivery(adapter.clone(), DELIVERY_BUDGET);
        let handle = delivery.spawn().unwrap();
        enqueue.send("{\"secret\":\"payload-one\"}".into()).unwrap();
        enqueue.send("{\"secret\":\"payload-two\"}".into()).unwrap();

        let report = handle.shutdown().await.unwrap();

        assert_eq!(report.accepted(), 2);
        assert_eq!(report.delivered(), 1);
        assert_eq!(report.retries(), 0);
        assert_eq!(report.failures()[0].attempts(), 1);
        assert_eq!(report.failures()[0].reason(), &DeliveryFailureReason::HttpStatus(400));
        assert!(!format!("{report:?}").contains("payload"));
        assert_eq!(adapter.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn retries_transient_failures_and_preserves_following_work() {
        let adapter = Arc::new(ScriptedAdapter::new([
            SendOutcome::Network,
            response(StatusCode::REQUEST_TIMEOUT),
            response(StatusCode::INTERNAL_SERVER_ERROR),
            response(StatusCode::NO_CONTENT),
            response(StatusCode::NO_CONTENT),
        ]));
        let (enqueue, delivery) = test_delivery(adapter.clone(), DELIVERY_BUDGET);
        let handle = delivery.spawn().unwrap();
        enqueue.send("first".into()).unwrap();
        enqueue.send("second".into()).unwrap();

        let report = handle.shutdown().await.unwrap();

        assert_eq!(report.accepted(), 2);
        assert_eq!(report.delivered(), 2);
        assert_eq!(report.retries(), 3);
        assert!(report.failures().is_empty());
        assert_eq!(
            *adapter.bodies.lock().unwrap(),
            ["first", "first", "first", "first", "second"]
        );
        let instants = adapter.instants.lock().unwrap();
        assert_eq!(instants[1].duration_since(instants[0]), Duration::from_millis(100));
        assert_eq!(instants[2].duration_since(instants[1]), Duration::from_millis(200));
        assert_eq!(instants[3].duration_since(instants[2]), Duration::from_millis(400));
        assert_eq!(instants[4].duration_since(instants[3]), Duration::ZERO);
    }

    #[tokio::test(start_paused = true)]
    async fn honors_retry_after_within_budget() {
        let adapter = Arc::new(ScriptedAdapter::new([
            SendOutcome::Response {
                status: StatusCode::TOO_MANY_REQUESTS,
                retry_after: Some(Duration::from_secs(30)),
            },
            response(StatusCode::NO_CONTENT),
        ]));
        let (enqueue, delivery) = test_delivery(adapter, DELIVERY_BUDGET);
        let handle = delivery.spawn().unwrap();
        enqueue.send("message".into()).unwrap();

        let report = handle.shutdown().await.unwrap();

        assert_eq!(report.delivered(), 1);
        assert_eq!(report.retries(), 1);
    }

    #[tokio::test]
    async fn rejects_retry_after_beyond_budget() {
        let adapter = Arc::new(ScriptedAdapter::new([SendOutcome::Response {
            status: StatusCode::TOO_MANY_REQUESTS,
            retry_after: Some(Duration::from_secs(121)),
        }]));
        let (enqueue, delivery) = test_delivery(adapter, DELIVERY_BUDGET);
        let handle = delivery.spawn().unwrap();
        enqueue.send("message".into()).unwrap();

        let report = handle.shutdown().await.unwrap();

        assert_eq!(report.retries(), 0);
        assert_eq!(
            report.failures()[0].reason(),
            &DeliveryFailureReason::RetryBudgetExceeded
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_slow_http_attempt_cannot_exceed_the_delivery_budget() {
        let adapter = Arc::new(
            ScriptedAdapter::new([response(StatusCode::NO_CONTENT)])
                .with_delays([DELIVERY_BUDGET + Duration::from_secs(1)]),
        );
        let (enqueue, delivery) = test_delivery(adapter, DELIVERY_BUDGET);
        let handle = delivery.spawn().unwrap();
        enqueue.send("message".into()).unwrap();

        let report = handle.shutdown().await.unwrap();

        assert_eq!(report.accepted(), 1);
        assert_eq!(report.delivered(), 0);
        assert_eq!(report.retries(), 0);
        assert_eq!(report.failures()[0].attempts(), 1);
        assert_eq!(
            report.failures()[0].reason(),
            &DeliveryFailureReason::RetryBudgetExceeded
        );
    }

    #[tokio::test(start_paused = true)]
    async fn exhaustion_does_not_prevent_the_next_message() {
        let adapter = Arc::new(ScriptedAdapter::new([
            response(StatusCode::SERVICE_UNAVAILABLE),
            response(StatusCode::SERVICE_UNAVAILABLE),
            response(StatusCode::SERVICE_UNAVAILABLE),
            response(StatusCode::SERVICE_UNAVAILABLE),
            response(StatusCode::SERVICE_UNAVAILABLE),
            response(StatusCode::NO_CONTENT),
        ]));
        let (enqueue, delivery) = test_delivery(adapter, DELIVERY_BUDGET);
        let handle = delivery.spawn().unwrap();
        enqueue.send("first".into()).unwrap();
        enqueue.send("second".into()).unwrap();

        let report = handle.shutdown().await.unwrap();

        assert_eq!(report.accepted(), 2);
        assert_eq!(report.delivered(), 1);
        assert_eq!(report.retries(), 4);
        assert_eq!(report.failures()[0].attempts(), 5);
    }

    #[test]
    fn spawning_requires_a_runtime() {
        let (_enqueue, delivery) = delivery_channel(WebhookUrl::parse("https://example.com/hook").unwrap());
        assert!(matches!(delivery.spawn(), Err(Error::RuntimeUnavailable)));
    }
}
