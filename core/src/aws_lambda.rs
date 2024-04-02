// use lambda_extension::{tracing, Error, Extension, LambdaLog, LambdaLogRecord, Service, SharedService};
// use std::{future::Future, pin::Pin, task::Poll};
// use aws_sdk_lambda::primitives::Blob;
// use tracing::span::Record;
//
// #[derive(Clone)]
// struct FirehoseLogsProcessor {
// }
//
// impl FirehoseLogsProcessor {
//     pub fn new() -> Self {
//     }
// }
//
// /// Implementation of the actual log processor
// ///
// /// This receives a `Vec<LambdaLog>` whenever there are new log entries available.
// impl Service<Vec<LambdaLog>> for FirehoseLogsProcessor {
//     type Response = ();
//     type Error = Error;
//     type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
//
//     fn poll_ready(&mut self, _cx: &mut core::task::Context<'_>) -> core::task::Poll<Result<(), Self::Error>> {
//         Poll::Ready(Ok(()))
//     }
//
//     fn call(&mut self, logs: Vec<LambdaLog>) -> Self::Future {
//         let mut records = Vec::with_capacity(logs.len());
//
//         for log in logs {
//             match log.record {
//                 LambdaLogRecord::Function(record) => {
//                     records.push(Record::builder().data(Blob::new(record.as_bytes())).build())
//                 }
//                 _ => unreachable!(),
//             }
//         }
//
//         // let fut = self
//         //     .client
//         //     .put_record_batch()
//         //     .set_records(Some(records))
//         //     .delivery_stream_name(std::env::var("KINESIS_DELIVERY_STREAM").unwrap())
//         //     .send();
//         //
//         // Box::pin(async move {
//         //     let _ = fut.await?;
//         //     Ok(())
//         // })
//         todo!()
//     }
// }
//
// #[tokio::main]
// async fn main() -> Result<(), Error> {
//     // required to enable CloudWatch error logging by the runtime
//     tracing::init_default_subscriber();
//
//     let client = aws_config::load_defaults(aws_config::BehaviorVersion::v2023_11_09()).await;
//     let logs_processor = SharedService::new(FirehoseLogsProcessor::new());
//
//     Extension::new()
//         .with_log_types(&["function"])
//         .with_logs_processor(logs_processor)
//         .run()
//         .await?;
//
//     Ok(())
// }