use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::{Sink, SinkExt, Stream, StreamExt};
use tokio::sync::Mutex as AsyncMutex;

use crate::{Frame, INLINE_PAYLOAD_SIZE, INLINE_PAYLOAD_SLOT, MsgDescHot, Payload, TransportError};

use super::TransportBackend;

#[cfg(not(target_arch = "wasm32"))]
use tokio_tungstenite::tungstenite::{Error as WsError, Message};

/// Size of MsgDescHot in bytes (must be 64).
const DESC_SIZE: usize = 64;
const _: () = assert!(std::mem::size_of::<MsgDescHot>() == DESC_SIZE);

fn desc_to_bytes(desc: &MsgDescHot) -> [u8; DESC_SIZE] {
    unsafe { std::mem::transmute_copy(desc) }
}

fn bytes_to_desc(bytes: &[u8; DESC_SIZE]) -> MsgDescHot {
    unsafe { std::mem::transmute_copy(bytes) }
}

#[derive(Clone)]
pub struct WebSocketTransport {
    inner: Arc<WebSocketInner>,
}

impl std::fmt::Debug for WebSocketTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebSocketTransport")
            .field("closed", &self.inner.closed.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

#[cfg(not(target_arch = "wasm32"))]
type DynSink = Box<dyn Sink<Message, Error = WsError> + Unpin + Send>;
#[cfg(not(target_arch = "wasm32"))]
type DynStream = Box<dyn Stream<Item = Result<Message, WsError>> + Unpin + Send>;

struct WebSocketInner {
    #[cfg(not(target_arch = "wasm32"))]
    sink: AsyncMutex<DynSink>,
    #[cfg(not(target_arch = "wasm32"))]
    stream: AsyncMutex<DynStream>,
    closed: AtomicBool,
}

impl WebSocketTransport {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new<S>(ws: tokio_tungstenite::WebSocketStream<S>) -> Self
    where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        let (sink, stream) = ws.split();
        Self {
            inner: Arc::new(WebSocketInner {
                sink: AsyncMutex::new(Box::new(sink)),
                stream: AsyncMutex::new(Box::new(stream)),
                closed: AtomicBool::new(false),
            }),
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn pair() -> (Self, Self) {
        let (client_stream, server_stream) = tokio::io::duplex(65536);

        let (ws_a, ws_b) = tokio::join!(
            async {
                tokio_tungstenite::client_async("ws://localhost/", client_stream)
                    .await
                    .expect("client handshake failed")
                    .0
            },
            async {
                tokio_tungstenite::accept_async(server_stream)
                    .await
                    .expect("server handshake failed")
            }
        );

        (Self::new(ws_a), Self::new(ws_b))
    }

    fn is_closed_inner(&self) -> bool {
        self.inner.closed.load(Ordering::Acquire)
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn ws_err(e: WsError, context: &'static str) -> TransportError {
    TransportError::Io(std::io::Error::other(format!("{context}: {e}")))
}

impl TransportBackend for WebSocketTransport {
    async fn send_frame(&self, frame: Frame) -> Result<(), TransportError> {
        if self.is_closed_inner() {
            return Err(TransportError::Closed);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let payload = frame.payload_bytes();
            let mut data = Vec::with_capacity(DESC_SIZE + payload.len());
            data.extend_from_slice(&desc_to_bytes(&frame.desc));
            data.extend_from_slice(payload);

            let mut sink = self.inner.sink.lock().await;
            sink.send(Message::Binary(data.into()))
                .await
                .map_err(|e| ws_err(e, "websocket send"))?;
            return Ok(());
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = frame;
            Err(TransportError::Io(std::io::Error::other(
                "websocket transport is not supported on wasm32 in rapace-core yet",
            )))
        }
    }

    async fn recv_frame(&self) -> Result<Frame, TransportError> {
        if self.is_closed_inner() {
            return Err(TransportError::Closed);
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let mut stream = self.inner.stream.lock().await;

            loop {
                let msg = stream
                    .next()
                    .await
                    .ok_or(TransportError::Closed)?
                    .map_err(|e| ws_err(e, "websocket recv"))?;

                match msg {
                    Message::Close(_) => {
                        self.inner.closed.store(true, Ordering::Release);
                        return Err(TransportError::Closed);
                    }
                    Message::Ping(_)
                    | Message::Pong(_)
                    | Message::Text(_)
                    | Message::Frame(_) => {
                        continue;
                    }
                    Message::Binary(data) => {
                        let data: Vec<u8> = data.into();
                        if data.len() < DESC_SIZE {
                            return Err(TransportError::Io(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                format!("frame too small: {} < {}", data.len(), DESC_SIZE),
                            )));
                        }

                        let desc_bytes: [u8; DESC_SIZE] = data[..DESC_SIZE].try_into().unwrap();
                        let mut desc = bytes_to_desc(&desc_bytes);

                        let payload = data[DESC_SIZE..].to_vec();
                        let payload_len = payload.len();
                        desc.payload_len = payload_len as u32;

                        if payload_len <= INLINE_PAYLOAD_SIZE {
                            desc.payload_slot = INLINE_PAYLOAD_SLOT;
                            desc.payload_generation = 0;
                            desc.payload_offset = 0;
                            desc.inline_payload[..payload_len].copy_from_slice(&payload);
                            return Ok(Frame {
                                desc,
                                payload: Payload::Inline,
                            });
                        }

                        desc.payload_slot = 0;
                        desc.payload_generation = 0;
                        desc.payload_offset = 0;
                        return Ok(Frame {
                            desc,
                            payload: Payload::Owned(payload),
                        });
                    }
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            Err(TransportError::Io(std::io::Error::other(
                "websocket transport is not supported on wasm32 in rapace-core yet",
            )))
        }
    }

    fn close(&self) {
        self.inner.closed.store(true, Ordering::Release);

        #[cfg(not(target_arch = "wasm32"))]
        {
            let inner = self.inner.clone();
            tokio::spawn(async move {
                let mut sink = inner.sink.lock().await;
                let _ = sink.send(Message::Close(None)).await;
            });
        }
    }

    fn is_closed(&self) -> bool {
        self.is_closed_inner()
    }
}
