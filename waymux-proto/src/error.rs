// SPDX-License-Identifier: Apache-2.0

//! Codec error type for all encode/decode failures in waymux-proto.

use thiserror::Error;

/// Errors produced by the WFP/WIP codec during encode or decode operations.
#[derive(Debug, Error)]
pub enum CodecError {
    /// Received a message-type discriminant byte not defined in the protocol.
    #[error("unknown WFP message type: 0x{0:02X}")]
    UnknownMessageType(u8),

    /// Received an encoding identifier not defined in [`crate::FrameEncoding`].
    #[error("unknown frame encoding: 0x{0:02X}")]
    UnknownEncoding(u8),

    /// Received a disconnect reason code not defined in the protocol.
    #[error("unknown disconnect reason: 0x{0:02X}")]
    UnknownDisconnectReason(u8),

    /// Received a button-state byte not defined in the protocol.
    #[error("unknown button state: 0x{0:02X}")]
    UnknownButtonState(u8),

    /// Received a pointer-axis byte not defined in the protocol.
    #[error("unknown pointer axis: 0x{0:02X}")]
    UnknownPointerAxis(u8),

    /// The buffer contained fewer bytes than needed to complete a decode.
    #[error("insufficient data: needed {needed} bytes, have {available}")]
    InsufficientData {
        /// Number of bytes required to proceed.
        needed: usize,
        /// Number of bytes actually available.
        available: usize,
    },

    /// The outer length-prefix declared a frame size that does not match the
    /// number of payload bytes available inside the frame.
    #[error("invalid frame length: declared {declared}, available {available}")]
    InvalidFrameLength {
        /// Length declared in the outer `u32` prefix field.
        declared: u32,
        /// Number of bytes actually available in the payload.
        available: usize,
    },

    /// Received a WIP message-type discriminant byte not defined in the protocol.
    #[error("unknown WIP message type: 0x{0:02X}")]
    UnknownWipMessageType(u8),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_unknown_message_type() {
        let e = CodecError::UnknownMessageType(0xAB);
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_unknown_encoding() {
        let e = CodecError::UnknownEncoding(0xFF);
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_unknown_disconnect_reason() {
        let e = CodecError::UnknownDisconnectReason(0x99);
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_unknown_button_state() {
        let e = CodecError::UnknownButtonState(0x05);
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_unknown_pointer_axis() {
        let e = CodecError::UnknownPointerAxis(0x07);
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_insufficient_data() {
        let e = CodecError::InsufficientData { needed: 10, available: 3 };
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_invalid_frame_length() {
        let e = CodecError::InvalidFrameLength { declared: 100, available: 50 };
        assert!(!e.to_string().is_empty());
    }

    #[test]
    fn display_unknown_wip_message_type() {
        let e = CodecError::UnknownWipMessageType(0x20);
        assert!(!e.to_string().is_empty());
    }
}
