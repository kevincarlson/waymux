//! Waymux Frame Protocol (WFP) message types: Bridge → Client.
//!
//! Each variant's binary body layout is documented inline and matches
//! `waymux-proto/docs/spec.md`. The body begins with a one-byte type
//! discriminant; all multi-byte integers are little-endian.

use bytes::{BufMut, Bytes, BytesMut};

use crate::codec::{
    put_len_prefixed, read_f32, read_len_prefixed, read_u8, read_u16, read_u32, read_u64,
};
use crate::encoding::FrameEncoding;
use crate::error::CodecError;

/// A complete message from the Waymux Bridge to the Waymux Client.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum WfpMessage {
    /// Full frame update covering the entire display area.
    FrameFull(FrameFullMsg),
    /// Partial frame update covering one or more damage rectangles.
    FrameDamage(FrameDamageMsg),
    /// Display geometry notification (sent on connect and on resize).
    DisplayInfo(DisplayInfoMsg),
    /// Keepalive ping; the client must respond with a WIP `Pong`.
    Ping {
        /// Monotonic sequence number the client echoes back.
        sequence: u64,
    },
    /// Server-initiated disconnect with a reason code.
    Disconnect {
        /// Why the bridge is closing the session.
        reason: DisconnectReason,
    },
}

/// Body of [`WfpMessage::FrameFull`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameFullMsg {
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Encoding of `data`.
    pub encoding: FrameEncoding,
    /// Encoded pixel payload for the whole frame.
    pub data: Bytes,
}

/// Body of [`WfpMessage::FrameDamage`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct FrameDamageMsg {
    /// Encoding of `data`.
    pub encoding: FrameEncoding,
    /// Rectangles, in display pixels, that `data` updates.
    pub regions: Vec<DamageRegion>,
    /// Encoded pixel payload covering the damaged regions.
    pub data: Bytes,
}

/// Body of [`WfpMessage::DisplayInfo`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DisplayInfoMsg {
    /// Logical display width in pixels.
    pub width: u32,
    /// Logical display height in pixels.
    pub height: u32,
    /// Output scale factor (physical / logical pixels).
    pub scale_factor: f32,
    /// Refresh rate in hertz.
    pub refresh_hz: f32,
}

/// A single rectangular damage region in display pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DamageRegion {
    /// Left edge in pixels.
    pub x: u32,
    /// Top edge in pixels.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

/// Reason accompanying a [`WfpMessage::Disconnect`].
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DisconnectReason {
    /// The bridge is shutting down normally.
    ServerShutdown = 0x00,
    /// The client violated the protocol.
    ProtocolError = 0x01,
    /// The upstream compositor connection was lost.
    CompositorLost = 0x02,
}

impl DisconnectReason {
    /// Returns the wire discriminant byte.
    #[must_use]
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Parses a wire discriminant byte.
    ///
    /// # Errors
    /// Returns [`CodecError::UnknownDisconnectReason`] for an unknown code.
    pub fn from_u8(value: u8) -> Result<Self, CodecError> {
        match value {
            0x00 => Ok(Self::ServerShutdown),
            0x01 => Ok(Self::ProtocolError),
            0x02 => Ok(Self::CompositorLost),
            other => Err(CodecError::UnknownDisconnectReason(other)),
        }
    }
}

impl WfpMessage {
    /// Serialises the message body (discriminant + fields) into `buf`.
    pub(crate) fn encode(&self, buf: &mut BytesMut) -> Result<(), CodecError> {
        match self {
            // [0x01][width u32][height u32][encoding u8][data_len u32][data]
            WfpMessage::FrameFull(m) => {
                buf.put_u8(0x01);
                buf.put_u32_le(m.width);
                buf.put_u32_le(m.height);
                buf.put_u8(m.encoding.to_u8());
                put_len_prefixed(buf, &m.data)?;
            }
            // [0x02][encoding u8][region_count u16] regions... [data_len u32][data]
            WfpMessage::FrameDamage(m) => {
                buf.put_u8(0x02);
                buf.put_u8(m.encoding.to_u8());
                let count = u16::try_from(m.regions.len())
                    .map_err(|_| CodecError::TooManyRegions(m.regions.len()))?;
                buf.put_u16_le(count);
                for r in &m.regions {
                    buf.put_u32_le(r.x);
                    buf.put_u32_le(r.y);
                    buf.put_u32_le(r.width);
                    buf.put_u32_le(r.height);
                }
                put_len_prefixed(buf, &m.data)?;
            }
            // [0x03][width u32][height u32][scale f32][refresh f32]
            WfpMessage::DisplayInfo(m) => {
                buf.put_u8(0x03);
                buf.put_u32_le(m.width);
                buf.put_u32_le(m.height);
                buf.put_f32_le(m.scale_factor);
                buf.put_f32_le(m.refresh_hz);
            }
            // [0x10][sequence u64]
            WfpMessage::Ping { sequence } => {
                buf.put_u8(0x10);
                buf.put_u64_le(*sequence);
            }
            // [0xFF][reason u8]
            WfpMessage::Disconnect { reason } => {
                buf.put_u8(0xff);
                buf.put_u8(reason.to_u8());
            }
        }
        Ok(())
    }

    /// Parses a message body (after framing) from `buf`.
    pub(crate) fn decode(buf: &mut Bytes) -> Result<Self, CodecError> {
        match read_u8(buf)? {
            0x01 => {
                let width = read_u32(buf)?;
                let height = read_u32(buf)?;
                let encoding = FrameEncoding::from_u8(read_u8(buf)?)?;
                let data = read_len_prefixed(buf)?;
                Ok(WfpMessage::FrameFull(FrameFullMsg {
                    width,
                    height,
                    encoding,
                    data,
                }))
            }
            0x02 => {
                let encoding = FrameEncoding::from_u8(read_u8(buf)?)?;
                let count = read_u16(buf)? as usize;
                let mut regions = Vec::with_capacity(count);
                for _ in 0..count {
                    regions.push(DamageRegion {
                        x: read_u32(buf)?,
                        y: read_u32(buf)?,
                        width: read_u32(buf)?,
                        height: read_u32(buf)?,
                    });
                }
                let data = read_len_prefixed(buf)?;
                Ok(WfpMessage::FrameDamage(FrameDamageMsg {
                    encoding,
                    regions,
                    data,
                }))
            }
            0x03 => Ok(WfpMessage::DisplayInfo(DisplayInfoMsg {
                width: read_u32(buf)?,
                height: read_u32(buf)?,
                scale_factor: read_f32(buf)?,
                refresh_hz: read_f32(buf)?,
            })),
            0x10 => Ok(WfpMessage::Ping {
                sequence: read_u64(buf)?,
            }),
            0xff => Ok(WfpMessage::Disconnect {
                reason: DisconnectReason::from_u8(read_u8(buf)?)?,
            }),
            other => Err(CodecError::UnknownWfpType(other)),
        }
    }
}
