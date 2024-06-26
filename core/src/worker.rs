use std::fmt::Debug;
use std::sync::Arc;

use tokio::task::JoinHandle;
use debug_print::debug_println;
use tokio::sync::Mutex;

use crate::{ChannelReceiver, ChannelSender, WebhookMessage};

/// Maximum number of retries for failed requests
const MAX_RETRIES: usize = 10;

/// This worker manages a background async task that schedules the network requests to send traces
/// to the Discord on the running tokio runtime.
///
/// Ensure to invoke `.startup()` before, and `.teardown()` after, your application code runs. This
/// is required to ensure proper initialization and shutdown.
///
/// `tracing-layer-discord` synchronously generates payloads to send to the Discord API using the
/// tracing events from the global subscriber. However, all network requests are offloaded onto
/// an unbuffered channel and processed by a provided future acting as an asynchronous worker.
#[derive(Clone)]
pub struct BackgroundWorker {
    pub(crate) sender: ChannelSender,
    pub(crate) handle: Arc<Mutex<Option<JoinHandle<()>>>>,
}

impl BackgroundWorker {
    /// Initiate the worker's shutdown sequence.
    ///
    /// Without invoking`.teardown()`, your application may exit before all Discord messages can be
    /// sent.
    pub async fn shutdown(self) {
        match self.sender.send(WorkerMessage::Shutdown) {
            Ok(..) => {
                debug_println!("webhook message worker shutdown");
            }
            Err(e) => {
                println!("ERROR: failed to send shutdown message to webhook message worker: {}", e);
            }
        }
        let mut guard = self.handle.lock().await;
        if let Some(handle) = guard.take() {
            let _ = handle.await;
        } else {
            println!("ERROR: async task handle to webhook message worker has been already dropped");
        }
    }

}

/// A command sent to a worker containing a new message that should be sent to a webhook endpoint.
#[derive(Debug)]
pub enum WorkerMessage {
    Data(Box<dyn WebhookMessage>),
    Shutdown,
}

/// Provides a background worker task that sends the messages generated by the
/// layer.
pub(crate) async fn worker(mut rx: ChannelReceiver) {
    let client = reqwest::Client::new();
    while let Some(message) = rx.recv().await {
        match message {
            WorkerMessage::Data(payload) => {
                let webhook_url = payload.webhook_url();
                let payload_json = payload.serialize();
                println!("sending discord message: {}", &payload_json);

                let mut retries = 0;
                while retries < MAX_RETRIES {
                    match client
                        .post(webhook_url)
                        .header("Content-Type", "application/json")
                        .body(payload_json.clone())
                        .send()
                        .await
                    {
                        Ok(res) => {
                            debug_println!("webhook message sent: {:?}", &res);
                            let res_text = res.text().await.unwrap();
                            debug_println!("webhook message response: {}", res_text);
                            break; // Success, break out of the retry loop
                        }
                        Err(e) => {
                            println!("ERROR: failed to send webhook message: {}", e);
                        }
                    };

                    // Exponential backoff - increase the delay between retries
                    let delay_ms = 2u64.pow(retries as u32) * 100;
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    retries += 1;
                }
            }
            WorkerMessage::Shutdown => {
                break;
            }
        }
    }
}
