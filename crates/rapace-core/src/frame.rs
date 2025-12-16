//! Unified frame representation.

use crate::MsgDescHot;

/// Payload storage for a frame.
#[derive(Debug)]
pub enum Payload {
    /// Payload bytes live inside `MsgDescHot::inline_payload`.
    Inline,
    /// Payload bytes are owned as a heap allocation.
    Owned(Vec<u8>),
}

impl Payload {
    /// Borrow the payload as a byte slice.
    pub fn as_slice<'a>(&'a self, desc: &'a MsgDescHot) -> &'a [u8] {
        match self {
            Payload::Inline => desc.inline_payload(),
            Payload::Owned(buf) => buf.as_slice(),
        }
    }

    /// Returns true if this payload is stored inline.
    pub fn is_inline(&self) -> bool {
        matches!(self, Payload::Inline)
    }
}

/// Owned frame for sending, receiving, or routing.
#[derive(Debug)]
pub struct Frame {
    /// The frame descriptor.
    pub desc: MsgDescHot,
    /// Payload storage for this frame.
    pub payload: Payload,
}

impl Frame {
    /// Create a new frame with no payload (inline empty).
    pub fn new(desc: MsgDescHot) -> Self {
        Self {
            desc,
            payload: Payload::Inline,
        }
    }

    /// Create a frame with inline payload.
    pub fn with_inline_payload(mut desc: MsgDescHot, payload: &[u8]) -> Option<Self> {
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
            payload: Payload::Inline,
        })
    }

    /// Create a frame with an owned payload allocation.
    pub fn with_payload(mut desc: MsgDescHot, payload: Vec<u8>) -> Self {
        desc.payload_slot = 0;
        desc.payload_generation = 0;
        desc.payload_offset = 0;
        desc.payload_len = payload.len() as u32;
        Self {
            desc,
            payload: Payload::Owned(payload),
        }
    }

    /// Borrow the payload as bytes.
    pub fn payload_bytes(&self) -> &[u8] {
        self.payload.as_slice(&self.desc)
    }
}
