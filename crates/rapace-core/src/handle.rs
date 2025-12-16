//! Unified transport enum.
//!
//! Previously rapace exposed a `TransportHandle` trait implemented by each
//! transport crate. We're moving toward a single `Transport` enum that carries
//! concrete transport implementations (mem, stream, shm, websocket) behind
//! feature flags so higher layers no longer need generics to talk to a
//! transport. For now the variants contain stub structs; the real transports
//! will migrate here as the merge progresses.

use std::ops::Deref;

use crate::TransportError;

/// Transport variants (stubbed for now).
#[derive(Clone, Debug)]
pub enum Transport {
    /// In-process transport (feature `mem`).
    #[cfg(feature = "mem")]
    Mem(MemTransportStub),
    /// Stream transport (feature `stream`).
    #[cfg(feature = "stream")]
    Stream(StreamTransportStub),
    /// Shared-memory transport (feature `shm`).
    #[cfg(feature = "shm")]
    Shm(ShmTransportStub),
    /// WebSocket transport (feature `websocket`).
    #[cfg(feature = "websocket")]
    WebSocket(WebSocketTransportStub),
}

impl Transport {
    /// Send a frame using the selected transport.
    pub async fn send_frame(
        &self,
        _frame: impl Into<SendFrame<Vec<u8>>> + Send + 'static,
    ) -> Result<(), TransportError> {
        todo!("Transport::send_frame: wire up concrete transport variants")
    }

    /// Receive the next frame.
    pub async fn recv_frame(&self) -> Result<crate::RecvFrame<Vec<u8>>, TransportError> {
        todo!("Transport::recv_frame: wire up concrete transport variants")
    }

    /// Initiate shutdown.
    pub fn close(&self) {
        todo!("Transport::close: wire up concrete transport variants")
    }

    /// Returns true if the underlying transport is closed.
    pub fn is_closed(&self) -> bool {
        todo!("Transport::is_closed: wire up concrete transport variants")
    }
}

#[cfg(feature = "mem")]
#[derive(Clone, Debug)]
pub struct MemTransportStub;

#[cfg(feature = "stream")]
#[derive(Clone, Debug)]
pub struct StreamTransportStub;

#[cfg(feature = "shm")]
#[derive(Clone, Debug)]
pub struct ShmTransportStub;

#[cfg(feature = "websocket")]
#[derive(Clone, Debug)]
pub struct WebSocketTransportStub;

/// Frame for sending with generic payload.
#[derive(Debug, Clone)]
pub struct SendFrame<P> {
    /// The frame descriptor.
    pub desc: crate::MsgDescHot,
    /// Optional payload (None if inline or empty).
    pub payload: Option<P>,
}

impl<P: Deref<Target = [u8]>> SendFrame<P> {
    /// Create a new send frame with the given descriptor.
    pub fn new(desc: crate::MsgDescHot) -> Self {
        Self {
            desc,
            payload: None,
        }
    }

    /// Create a frame with external payload.
    pub fn with_payload(mut desc: crate::MsgDescHot, payload: P) -> Self {
        desc.payload_slot = 0;
        desc.payload_len = payload.deref().len() as u32;
        Self {
            desc,
            payload: Some(payload),
        }
    }

    /// Get the payload bytes.
    pub fn payload_bytes(&self) -> &[u8] {
        if self.desc.is_inline() {
            self.desc.inline_payload()
        } else if let Some(ref p) = self.payload {
            p.deref()
        } else {
            &[]
        }
    }
}

impl SendFrame<Vec<u8>> {
    /// Create a frame with inline payload.
    pub fn with_inline_payload(mut desc: crate::MsgDescHot, payload: &[u8]) -> Option<Self> {
        if payload.len() > crate::INLINE_PAYLOAD_SIZE {
            return None;
        }
        desc.payload_slot = crate::INLINE_PAYLOAD_SLOT;
        desc.payload_generation = 0;
        desc.payload_offset = 0;
        desc.payload_len = payload.len() as u32;
        desc.inline_payload[..payload.len()].copy_from_slice(payload);
        Some(Self {
            desc,
            payload: None,
        })
    }
}

/// Convert an owned Frame to a SendFrame.
impl From<crate::Frame> for SendFrame<Vec<u8>> {
    fn from(frame: crate::Frame) -> Self {
        Self {
            desc: frame.desc,
            payload: frame.payload,
        }
    }
}

/// Convert a borrowed Frame to a SendFrame (clones payload if present).
impl From<&crate::Frame> for SendFrame<Vec<u8>> {
    fn from(frame: &crate::Frame) -> Self {
        Self {
            desc: frame.desc,
            payload: frame.payload.clone(),
        }
    }
}

/// Convert a SendFrame back to a Frame.
impl From<SendFrame<Vec<u8>>> for crate::Frame {
    fn from(send_frame: SendFrame<Vec<u8>>) -> Self {
        Self {
            desc: send_frame.desc,
            payload: send_frame.payload,
        }
    }
}

// QoS types for future use

/// Quality of service class for prioritization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QoSClass {
    /// Control frames (ping, close, cancel) - highest priority.
    Control,
    /// High priority data.
    High,
    /// Normal priority (default).
    #[default]
    Normal,
    /// Bulk/background data - lowest priority.
    Bulk,
}

/// Message wrapper with QoS metadata.
#[derive(Debug, Clone)]
pub struct Prioritized<T> {
    /// The QoS class.
    pub class: QoSClass,
    /// The wrapped item.
    pub item: T,
}

impl<T> Prioritized<T> {
    /// Create a new prioritized item with Normal class.
    pub fn normal(item: T) -> Self {
        Self {
            class: QoSClass::Normal,
            item,
        }
    }

    /// Create a new prioritized item with the given class.
    pub fn with_class(class: QoSClass, item: T) -> Self {
        Self { class, item }
    }
}
