// SPDX-License-Identifier: Apache-2.0

//! Waymux Frame Protocol (WFP) message types: Bridge → Client.

use bytes::Bytes;

use crate::error::CodecError;
use crate::encoding::FrameEncoding;

/// A complete message sent from the Waymux Bridge to the Waymux Client.
#[repr(u8)]
#[derive(Debug, Clone)]
pub enum WfpMessage {
    /// Full frame update covering the entire display area.
    FrameFull(FrameFullMsg) = 0x01,
    /// Partial frame update covering one or more damage rectangles.
    FrameDamage(FrameDamageMsg) = 0x02,
    /// Display geometry notification sent on connect and on every resize.
    DisplayInfo(DisplayInfoMsg) = 0x03,
    /// Keepalive ping; the client must respond with a WIP [`crate::WipMessage::Pong`].
    Ping {
        /// Monotonically increasing sequence number echoed in the Pong reply.
        sequence: u64,
    } = 0x10,
    /// Server-initiated disconnect carrying a machine-readable reason code.
    Disconnect {
        /// Reason the bridge is terminating the connection.
        reason: DisconnectReason,
    } = 0xFF,
}

impl PartialEq for WfpMessage {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (WfpMessage::FrameFull(a), WfpMessage::FrameFull(b)) => a == b,
            (WfpMessage::FrameDamage(a), WfpMessage::FrameDamage(b)) => a == b,
            (WfpMessage::DisplayInfo(a), WfpMessage::DisplayInfo(b)) => a == b,
            (WfpMessage::Ping { sequence: a }, WfpMessage::Ping { sequence: b }) => a == b,
            (WfpMessage::Disconnect { reason: a }, WfpMessage::Disconnect { reason: b }) => a == b,
            _ => false,
        }
    }
}

/// Payload for a [`WfpMessage::FrameFull`] message.
#[derive(Debug, Clone)]
pub struct FrameFullMsg {
    /// Logical pixel width of the full frame.
    pub width: u32,
    /// Logical pixel height of the full frame.
    pub height: u32,
    /// Encoding applied to [`Self::data`].
    pub encoding: FrameEncoding,
    /// Encoded frame bytes.
    pub data: Bytes,
}

impl PartialEq for FrameFullMsg {
    fn eq(&self, other: &Self) -> bool {
        self.width == other.width
            && self.height == other.height
            && self.encoding == other.encoding
            && self.data.as_ref() == other.data.as_ref()
    }
}

/// Payload for a [`WfpMessage::FrameDamage`] message.
#[derive(Debug, Clone)]
pub struct FrameDamageMsg {
    /// Encoding applied to [`Self::data`].
    pub encoding: FrameEncoding,
    /// Regions of the display that have changed since the last frame.
    pub regions: Vec<DamageRegion>,
    /// Encoded frame bytes covering the union of all damage regions.
    pub data: Bytes,
}

impl PartialEq for FrameDamageMsg {
    fn eq(&self, other: &Self) -> bool {
        self.encoding == other.encoding
            && self.regions == other.regions
            && self.data.as_ref() == other.data.as_ref()
    }
}

/// Payload for a [`WfpMessage::DisplayInfo`] message.
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfoMsg {
    /// Logical pixel width of the display.
    pub width: u32,
    /// Logical pixel height of the display.
    pub height: u32,
    /// Display scale factor (HiDPI multiplier, e.g. `2.0` for 2× scaling).
    pub scale_factor: f32,
    /// Display refresh rate in Hz.
    pub refresh_hz: f32,
}

/// A rectangular region of the display that has changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DamageRegion {
    /// Left edge in logical pixels.
    pub x: u32,
    /// Top edge in logical pixels.
    pub y: u32,
    /// Width in logical pixels.
    pub width: u32,
    /// Height in logical pixels.
    pub height: u32,
}

/// Reason codes for a [`WfpMessage::Disconnect`] message.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    /// The bridge is shutting down gracefully.
    ServerShutdown = 0x00,
    /// The client sent an invalid or unsupported message.
    ProtocolError = 0x01,
    /// The Wayland compositor the bridge was connected to has exited.
    CompositorLost = 0x02,
}

impl TryFrom<u8> for DisconnectReason {
    type Error = CodecError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(DisconnectReason::ServerShutdown),
            0x01 => Ok(DisconnectReason::ProtocolError),
            0x02 => Ok(DisconnectReason::CompositorLost),
            other => Err(CodecError::UnknownDisconnectReason(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_full_msg_construction() {
        let msg = FrameFullMsg {
            width: 1920,
            height: 1080,
            encoding: FrameEncoding::RawBgra8,
            data: Bytes::from_static(b"\x00\x01\x02\x03"),
        };
        let cloned = msg.clone();
        assert_eq!(cloned.width, 1920);
        assert_eq!(cloned.height, 1080);
        assert_eq!(cloned.encoding, FrameEncoding::RawBgra8);
    }

    #[test]
    fn frame_damage_msg_construction() {
        let msg = FrameDamageMsg {
            encoding: FrameEncoding::ZstdBgra8,
            regions: vec![DamageRegion { x: 0, y: 0, width: 100, height: 100 }],
            data: Bytes::from_static(b"\xDE\xAD"),
        };
        let cloned = msg.clone();
        assert_eq!(cloned.regions.len(), 1);
    }

    #[test]
    fn display_info_msg_construction() {
        let msg =
            DisplayInfoMsg { width: 2560, height: 1440, scale_factor: 2.0, refresh_hz: 144.0 };
        let cloned = msg.clone();
        assert_eq!(cloned.width, 2560);
    }

    #[test]
    fn disconnect_reason_try_from_all_valid() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(DisconnectReason::try_from(0x00)?, DisconnectReason::ServerShutdown);
        assert_eq!(DisconnectReason::try_from(0x01)?, DisconnectReason::ProtocolError);
        assert_eq!(DisconnectReason::try_from(0x02)?, DisconnectReason::CompositorLost);
        Ok(())
    }

    #[test]
    fn disconnect_reason_unknown_byte() {
        let result = DisconnectReason::try_from(0xFE);
        assert!(matches!(result, Err(CodecError::UnknownDisconnectReason(0xFE))));
    }

    #[test]
    fn wfp_message_partial_eq() {
        let a = WfpMessage::Ping { sequence: 42 };
        let b = WfpMessage::Ping { sequence: 42 };
        let c = WfpMessage::Ping { sequence: 99 };
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
