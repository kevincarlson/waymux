//! Binary encode/decode for [`WipMessage`] bodies.
//!
//! Each variant's body begins with a one-byte type discriminant; all
//! multi-byte integers are little-endian. See `waymux-proto/docs/spec.md`.

use bytes::{BufMut, Bytes, BytesMut};

use super::{
    ButtonState, KeyMsg, PointerAxis, PointerAxisMsg, PointerButtonMsg, PointerMotionMsg,
    StylusMsg, TouchPointMsg, WipMessage,
};
use crate::codec::{read_f32, read_u8, read_u32, read_u64};
use crate::error::CodecError;

impl WipMessage {
    /// Serialises the message body (discriminant + fields) into `buf`.
    pub(crate) fn encode(&self, buf: &mut BytesMut) -> Result<(), CodecError> {
        match self {
            WipMessage::PointerMotion(m) => {
                buf.put_u8(0x01);
                buf.put_f32_le(m.x);
                buf.put_f32_le(m.y);
                buf.put_u32_le(m.time_ms);
            }
            WipMessage::PointerButton(m) => {
                buf.put_u8(0x02);
                buf.put_u32_le(m.button);
                buf.put_u8(m.state.to_u8());
                buf.put_u32_le(m.time_ms);
            }
            WipMessage::PointerAxis(m) => {
                buf.put_u8(0x03);
                buf.put_u8(m.axis.to_u8());
                buf.put_f32_le(m.value);
                buf.put_u32_le(m.time_ms);
            }
            WipMessage::TouchDown(m) => {
                buf.put_u8(0x04);
                put_touch(buf, m);
            }
            WipMessage::TouchMotion(m) => {
                buf.put_u8(0x05);
                put_touch(buf, m);
            }
            WipMessage::TouchUp { id, time_ms } => {
                buf.put_u8(0x06);
                buf.put_u32_le(*id);
                buf.put_u32_le(*time_ms);
            }
            WipMessage::StylusDown(m) => {
                buf.put_u8(0x07);
                put_stylus(buf, m);
            }
            WipMessage::StylusMotion(m) => {
                buf.put_u8(0x08);
                put_stylus(buf, m);
            }
            WipMessage::StylusUp { time_ms } => {
                buf.put_u8(0x09);
                buf.put_u32_le(*time_ms);
            }
            WipMessage::KeyDown(m) => {
                buf.put_u8(0x0a);
                put_key(buf, m);
            }
            WipMessage::KeyUp(m) => {
                buf.put_u8(0x0b);
                put_key(buf, m);
            }
            WipMessage::Pong { sequence } => {
                buf.put_u8(0x10);
                buf.put_u64_le(*sequence);
            }
        }
        Ok(())
    }

    /// Parses a message body (after framing) from `buf`.
    pub(crate) fn decode(buf: &mut Bytes) -> Result<Self, CodecError> {
        match read_u8(buf)? {
            0x01 => Ok(WipMessage::PointerMotion(PointerMotionMsg {
                x: read_f32(buf)?,
                y: read_f32(buf)?,
                time_ms: read_u32(buf)?,
            })),
            0x02 => Ok(WipMessage::PointerButton(PointerButtonMsg {
                button: read_u32(buf)?,
                state: ButtonState::from_u8(read_u8(buf)?)?,
                time_ms: read_u32(buf)?,
            })),
            0x03 => Ok(WipMessage::PointerAxis(PointerAxisMsg {
                axis: PointerAxis::from_u8(read_u8(buf)?)?,
                value: read_f32(buf)?,
                time_ms: read_u32(buf)?,
            })),
            0x04 => Ok(WipMessage::TouchDown(read_touch(buf)?)),
            0x05 => Ok(WipMessage::TouchMotion(read_touch(buf)?)),
            0x06 => Ok(WipMessage::TouchUp {
                id: read_u32(buf)?,
                time_ms: read_u32(buf)?,
            }),
            0x07 => Ok(WipMessage::StylusDown(read_stylus(buf)?)),
            0x08 => Ok(WipMessage::StylusMotion(read_stylus(buf)?)),
            0x09 => Ok(WipMessage::StylusUp {
                time_ms: read_u32(buf)?,
            }),
            0x0a => Ok(WipMessage::KeyDown(read_key(buf)?)),
            0x0b => Ok(WipMessage::KeyUp(read_key(buf)?)),
            0x10 => Ok(WipMessage::Pong {
                sequence: read_u64(buf)?,
            }),
            other => Err(CodecError::UnknownWipType(other)),
        }
    }
}

fn put_touch(buf: &mut BytesMut, m: &TouchPointMsg) {
    buf.put_u32_le(m.id);
    buf.put_f32_le(m.x);
    buf.put_f32_le(m.y);
    buf.put_u32_le(m.time_ms);
}

fn read_touch(buf: &mut Bytes) -> Result<TouchPointMsg, CodecError> {
    Ok(TouchPointMsg {
        id: read_u32(buf)?,
        x: read_f32(buf)?,
        y: read_f32(buf)?,
        time_ms: read_u32(buf)?,
    })
}

fn put_stylus(buf: &mut BytesMut, m: &StylusMsg) {
    buf.put_f32_le(m.x);
    buf.put_f32_le(m.y);
    buf.put_f32_le(m.pressure);
    buf.put_f32_le(m.tilt_x);
    buf.put_f32_le(m.tilt_y);
    buf.put_u32_le(m.time_ms);
}

fn read_stylus(buf: &mut Bytes) -> Result<StylusMsg, CodecError> {
    Ok(StylusMsg {
        x: read_f32(buf)?,
        y: read_f32(buf)?,
        pressure: read_f32(buf)?,
        tilt_x: read_f32(buf)?,
        tilt_y: read_f32(buf)?,
        time_ms: read_u32(buf)?,
    })
}

fn put_key(buf: &mut BytesMut, m: &KeyMsg) {
    buf.put_u32_le(m.keycode);
    buf.put_u32_le(m.modifiers);
    buf.put_u32_le(m.time_ms);
}

fn read_key(buf: &mut Bytes) -> Result<KeyMsg, CodecError> {
    Ok(KeyMsg {
        keycode: read_u32(buf)?,
        modifiers: read_u32(buf)?,
        time_ms: read_u32(buf)?,
    })
}
