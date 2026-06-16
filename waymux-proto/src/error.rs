//! Error type produced by the WFP/WIP codec.

/// Errors that can occur while encoding or decoding Waymux protocol messages.
///
/// All variants are recoverable diagnostics: they describe either a malformed
/// byte stream from a peer or a value that cannot be represented within the
/// fixed-width wire framing. None of them indicate a bug in this crate.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CodecError {
    /// A WFP frame carried an unrecognised message-type discriminant.
    #[error("unknown WFP message type: {0:#04x}")]
    UnknownWfpType(u8),

    /// A WIP frame carried an unrecognised message-type discriminant.
    #[error("unknown WIP message type: {0:#04x}")]
    UnknownWipType(u8),

    /// A frame message referenced an unknown frame encoding id.
    #[error("unknown frame encoding id: {0:#04x}")]
    UnknownEncoding(u8),

    /// A pointer-button message carried an unknown button state.
    #[error("unknown button state: {0:#04x}")]
    UnknownButtonState(u8),

    /// A pointer-axis message carried an unknown axis selector.
    #[error("unknown pointer axis: {0:#04x}")]
    UnknownPointerAxis(u8),

    /// A disconnect message carried an unknown reason code.
    #[error("unknown disconnect reason: {0:#04x}")]
    UnknownDisconnectReason(u8),

    /// A payload exceeded the `u32` length the framing prefix can describe.
    #[error("payload of {0} bytes exceeds the maximum frame length")]
    PayloadTooLarge(usize),

    /// A `FrameDamage` message carried more regions than the `u16` count allows.
    #[error("damage region count {0} exceeds the u16 maximum")]
    TooManyRegions(usize),

    /// The frame body ended before all expected fields had been read.
    #[error("malformed frame: {0}")]
    Malformed(&'static str),
}
