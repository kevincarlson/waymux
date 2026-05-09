// SPDX-License-Identifier: Apache-2.0

//! WFP and WIP message decoding from length-prefixed frames.

use bytes::{Buf as _, BytesMut};

use crate::{
    encoding::FrameEncoding,
    error::CodecError,
    wfp::{DamageRegion, DisconnectReason, DisplayInfoMsg, FrameDamageMsg, FrameFullMsg, WfpMessage},
    wip::{
        ButtonState, KeyMsg, PointerAxis, PointerAxisMsg, PointerButtonMsg, PointerMotionMsg,
        StylusMsg, TouchPointMsg, WipMessage,
    },
};

/// Attempts to decode one [`WfpMessage`] from the front of `buf`.
///
/// Returns `Ok(None)` when `buf` contains fewer bytes than needed for a
/// complete frame (caller should buffer more data and retry). Returns `Err`
/// only for unrecoverable protocol violations (unknown discriminant, etc.).
pub fn decode_wfp(buf: &mut BytesMut) -> Result<Option<WfpMessage>, CodecError> {
    let Some(payload_len) = peek_payload_len(buf) else {
        return Ok(None);
    };
    let mut frame = buf.split_to(4 + payload_len);
    frame.advance(4); // consume the 4-byte length prefix

    let discriminant = frame.get_u8();
    match discriminant {
        0x01 => decode_frame_full(&mut frame).map(|m| Some(WfpMessage::FrameFull(m))),
        0x02 => decode_frame_damage(&mut frame).map(|m| Some(WfpMessage::FrameDamage(m))),
        0x03 => decode_display_info(&mut frame).map(|m| Some(WfpMessage::DisplayInfo(m))),
        0x10 => {
            require(frame.remaining(), 8)?;
            Ok(Some(WfpMessage::Ping { sequence: frame.get_u64_le() }))
        }
        0xFF => {
            require(frame.remaining(), 1)?;
            let reason = DisconnectReason::try_from(frame.get_u8())?;
            Ok(Some(WfpMessage::Disconnect { reason }))
        }
        other => Err(CodecError::UnknownMessageType(other)),
    }
}

/// Attempts to decode one [`WipMessage`] from the front of `buf`.
///
/// Returns `Ok(None)` when `buf` contains fewer bytes than needed for a
/// complete frame.
pub fn decode_wip(buf: &mut BytesMut) -> Result<Option<WipMessage>, CodecError> {
    let Some(payload_len) = peek_payload_len(buf) else {
        return Ok(None);
    };
    let mut frame = buf.split_to(4 + payload_len);
    frame.advance(4); // consume the 4-byte length prefix

    let discriminant = frame.get_u8();
    match discriminant {
        0x01 => decode_pointer_motion(&mut frame).map(|m| Some(WipMessage::PointerMotion(m))),
        0x02 => decode_pointer_button(&mut frame).map(|m| Some(WipMessage::PointerButton(m))),
        0x03 => decode_pointer_axis(&mut frame).map(|m| Some(WipMessage::PointerAxis(m))),
        0x04 => decode_touch_point(&mut frame).map(|m| Some(WipMessage::TouchDown(m))),
        0x05 => decode_touch_point(&mut frame).map(|m| Some(WipMessage::TouchMotion(m))),
        0x06 => {
            require(frame.remaining(), 8)?;
            let id = frame.get_u32_le();
            let time_ms = frame.get_u32_le();
            Ok(Some(WipMessage::TouchUp { id, time_ms }))
        }
        0x07 => decode_stylus(&mut frame).map(|m| Some(WipMessage::StylusDown(m))),
        0x08 => decode_stylus(&mut frame).map(|m| Some(WipMessage::StylusMotion(m))),
        0x09 => {
            require(frame.remaining(), 4)?;
            Ok(Some(WipMessage::StylusUp { time_ms: frame.get_u32_le() }))
        }
        0x0A => decode_key(&mut frame).map(|m| Some(WipMessage::KeyDown(m))),
        0x0B => decode_key(&mut frame).map(|m| Some(WipMessage::KeyUp(m))),
        0x10 => {
            require(frame.remaining(), 8)?;
            Ok(Some(WipMessage::Pong { sequence: frame.get_u64_le() }))
        }
        other => Err(CodecError::UnknownWipMessageType(other)),
    }
}

/// Peeks at the outer length prefix without consuming bytes.
///
/// Returns `None` if the buffer is too short for a complete frame.
fn peek_payload_len(buf: &BytesMut) -> Option<usize> {
    if buf.len() < 4 {
        return None;
    }
    let payload_len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + payload_len {
        return None;
    }
    Some(payload_len)
}

/// Returns `Err(InsufficientData)` if `available < needed`.
fn require(available: usize, needed: usize) -> Result<(), CodecError> {
    if available < needed {
        Err(CodecError::InsufficientData { needed, available })
    } else {
        Ok(())
    }
}

fn decode_frame_full(frame: &mut BytesMut) -> Result<FrameFullMsg, CodecError> {
    require(frame.remaining(), 13)?; // 4+4+1+4 minimum before data
    let width = frame.get_u32_le();
    let height = frame.get_u32_le();
    let encoding = FrameEncoding::try_from(frame.get_u8())?;
    let data_len = frame.get_u32_le() as usize;
    if frame.remaining() < data_len {
        return Err(CodecError::InvalidFrameLength {
            declared: data_len as u32,
            available: frame.remaining(),
        });
    }
    let data = frame.split_to(data_len).freeze();
    Ok(FrameFullMsg { width, height, encoding, data })
}

fn decode_frame_damage(frame: &mut BytesMut) -> Result<FrameDamageMsg, CodecError> {
    require(frame.remaining(), 7)?; // 1+2+4 minimum (zero regions)
    let encoding = FrameEncoding::try_from(frame.get_u8())?;
    let region_count = frame.get_u16_le() as usize;
    require(frame.remaining(), region_count * 16 + 4)?;
    let mut regions = Vec::with_capacity(region_count);
    for _ in 0..region_count {
        regions.push(DamageRegion {
            x: frame.get_u32_le(),
            y: frame.get_u32_le(),
            width: frame.get_u32_le(),
            height: frame.get_u32_le(),
        });
    }
    let data_len = frame.get_u32_le() as usize;
    if frame.remaining() < data_len {
        return Err(CodecError::InvalidFrameLength {
            declared: data_len as u32,
            available: frame.remaining(),
        });
    }
    let data = frame.split_to(data_len).freeze();
    Ok(FrameDamageMsg { encoding, regions, data })
}

fn decode_display_info(frame: &mut BytesMut) -> Result<DisplayInfoMsg, CodecError> {
    require(frame.remaining(), 16)?;
    Ok(DisplayInfoMsg {
        width: frame.get_u32_le(),
        height: frame.get_u32_le(),
        scale_factor: frame.get_f32_le(),
        refresh_hz: frame.get_f32_le(),
    })
}

fn decode_pointer_motion(frame: &mut BytesMut) -> Result<PointerMotionMsg, CodecError> {
    require(frame.remaining(), 12)?;
    Ok(PointerMotionMsg {
        x: frame.get_f32_le(),
        y: frame.get_f32_le(),
        time_ms: frame.get_u32_le(),
    })
}

fn decode_pointer_button(frame: &mut BytesMut) -> Result<PointerButtonMsg, CodecError> {
    require(frame.remaining(), 9)?;
    let button = frame.get_u32_le();
    let state = ButtonState::try_from(frame.get_u8())?;
    let time_ms = frame.get_u32_le();
    Ok(PointerButtonMsg { button, state, time_ms })
}

fn decode_pointer_axis(frame: &mut BytesMut) -> Result<PointerAxisMsg, CodecError> {
    require(frame.remaining(), 9)?;
    let axis = PointerAxis::try_from(frame.get_u8())?;
    let value = frame.get_f32_le();
    let time_ms = frame.get_u32_le();
    Ok(PointerAxisMsg { axis, value, time_ms })
}

fn decode_touch_point(frame: &mut BytesMut) -> Result<TouchPointMsg, CodecError> {
    require(frame.remaining(), 16)?;
    Ok(TouchPointMsg {
        id: frame.get_u32_le(),
        x: frame.get_f32_le(),
        y: frame.get_f32_le(),
        time_ms: frame.get_u32_le(),
    })
}

fn decode_stylus(frame: &mut BytesMut) -> Result<StylusMsg, CodecError> {
    require(frame.remaining(), 24)?;
    Ok(StylusMsg {
        x: frame.get_f32_le(),
        y: frame.get_f32_le(),
        pressure: frame.get_f32_le(),
        tilt_x: frame.get_f32_le(),
        tilt_y: frame.get_f32_le(),
        time_ms: frame.get_u32_le(),
    })
}

fn decode_key(frame: &mut BytesMut) -> Result<KeyMsg, CodecError> {
    require(frame.remaining(), 12)?;
    Ok(KeyMsg {
        keycode: frame.get_u32_le(),
        modifiers: frame.get_u32_le(),
        time_ms: frame.get_u32_le(),
    })
}
