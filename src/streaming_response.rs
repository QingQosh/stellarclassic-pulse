use axum::response::{IntoResponse, Response};
use axum::body::Body;
use futures::stream::{Stream, StreamExt};
use serde::Serialize;
use serde_json::json;
use std::pin::Pin;
use tracing::{debug, instrument};

pub type StreamingResultIterator<T> =
    Pin<Box<dyn Stream<Item = Result<T, sqlx::Error>> + Send + 'static>>;

pub struct StreamingJsonResponse<T: Serialize + Send> {
    stream: StreamingResultIterator<T>,
    buffer_size: usize,
}

impl<T: Serialize + Send + 'static> StreamingJsonResponse<T> {
    pub fn new(stream: StreamingResultIterator<T>) -> Self {
        Self {
            stream,
            buffer_size: 8192,
        }
    }

    pub fn with_buffer_size(stream: StreamingResultIterator<T>, buffer_size: usize) -> Self {
        Self { stream, buffer_size }
    }

    #[instrument(skip(self))]
    async fn to_streaming_body(self) -> Result<Body, String> {
        let (mut tx, rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1);

        tokio::spawn(async move {
            let mut stream = self.stream;
            let mut count: u64 = 0;

            if let Err(e) = tx.send(b"[".to_vec()).await {
                debug!(error = %e, "Failed to send opening bracket");
                return;
            }

            let mut first = true;
            while let Some(result) = stream.next().await {
                match result {
                    Ok(item) => {
                        match serde_json::to_vec(&item) {
                            Ok(json_bytes) => {
                                let mut chunk = Vec::with_capacity(json_bytes.len() + 2);
                                if !first {
                                    chunk.push(b',');
                                }
                                chunk.extend_from_slice(&json_bytes);

                                if let Err(e) = tx.send(chunk).await {
                                    debug!(error = %e, "Failed to send JSON chunk");
                                    return;
                                }

                                first = false;
                                count += 1;
                                crate::metrics::record_streaming_response_item_sent();
                            }
                            Err(e) => {
                                debug!(error = %e, "Serialization error in streaming response");
                                crate::metrics::record_streaming_response_error("serialization");
                            }
                        }
                    }
                    Err(e) => {
                        debug!(error = %e, "Database error in streaming response");
                        crate::metrics::record_streaming_response_error("database");
                    }
                }
            }

            if let Err(e) = tx.send(b"]".to_vec()).await {
                debug!(error = %e, "Failed to send closing bracket");
            }

            debug!(items_sent = count, "Streaming response completed");
            crate::metrics::record_streaming_response_completed(count);
        });

        let body = Body::from_stream(
            futures::stream::unfold(rx, |mut rx| async move {
                rx.recv().await.map(|bytes| (bytes, rx))
            })
            .map(Ok::<_, std::io::Error>),
        );

        Ok(body)
    }
}

impl<T: Serialize + Send + 'static> IntoResponse for StreamingJsonResponse<T> {
    fn into_response(self) -> Response {
        match futures::executor::block_on(self.to_streaming_body()) {
            Ok(body) => {
                (
                    axum::http::StatusCode::OK,
                    [
                        (
                            axum::http::header::CONTENT_TYPE,
                            "application/json",
                        ),
                        (
                            axum::http::header::TRANSFER_ENCODING,
                            "chunked",
                        ),
                    ],
                    body,
                )
                    .into_response()
            }
            Err(e) => {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    format!(r#"{{"error":"{}"}}"#, e),
                )
                    .into_response()
            }
        }
    }
}

pub struct StreamingStats {
    pub items_sent: u64,
    pub errors: u64,
    pub serialization_errors: u64,
    pub database_errors: u64,
}

pub fn create_streaming_response<T: Serialize + Send + 'static>(
    stream: StreamingResultIterator<T>,
) -> StreamingJsonResponse<T> {
    StreamingJsonResponse::new(stream)
}

pub fn create_streaming_response_with_buffer<T: Serialize + Send + 'static>(
    stream: StreamingResultIterator<T>,
    buffer_size: usize,
) -> StreamingJsonResponse<T> {
    StreamingJsonResponse::with_buffer_size(stream, buffer_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use futures::stream;

    #[test]
    fn streaming_response_creation() {
        let stream = Box::pin(stream::iter(vec![
            Ok::<serde_json::Value, sqlx::Error>(json!({"id": 1})),
            Ok(json!({"id": 2})),
        ]));

        let response = StreamingJsonResponse::new(stream);
        assert_eq!(response.buffer_size, 8192);
    }

    #[test]
    fn streaming_response_custom_buffer() {
        let stream = Box::pin(stream::iter(vec![
            Ok::<serde_json::Value, sqlx::Error>(json!({"id": 1})),
        ]));

        let response = StreamingJsonResponse::with_buffer_size(stream, 16384);
        assert_eq!(response.buffer_size, 16384);
    }
}
