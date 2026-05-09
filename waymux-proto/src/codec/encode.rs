// SPDX-License-Identifier: Apache-2.0

//! WFP and WIP message encoding into length-prefixed frames.

use bytes::BufMut as _;

use crate::{
    error::CodecError,
    wfp::{DamageRegion, DisplayInfoMsg, FrameDamageMsg, FrameFullMsg, WfpMessage},
    wip::{KeyMsg, PointerAxisMsg, PointerButtonMsg, PointerMotionMsg, StylusMsg, TouchPointMsg, WipMessage},
};

/// Encodes a [`WfpMessage`] as a length-prefixed frame appended to `buf`.
///
/// Frame format: `[u32 LE payload_len][payload bytes]` where the payload
/// begins with the one-byte message-type discriminant.
pub fn encode_wfp(msg: &WfpMessage, buf: &mut bytes::BytesMut) -> Result<(), CodecError> {
    let start = buf.len();
    buf.put_u32_le(0); // placeholder; patched below
    match msg {
        WfpMessage::FrameFull(m) => write_frame_full(m, buf),
        WfpMessage::FrameDamage(m) => write_frame_damage(m, buf),
        WfpMessage::DisplayInfo(m) => write_display_info(m, buf),
        WfpMessage::Ping { sequence } => {
            buf.put_u8(0x10);
            buf.put_u64_le(*sequence);
        }
        WfpMessage::Disconnect { reason } => {
            buf.put_u8(0xFF);
            buf.put_u8(*reason as u8);
        }
    }
    patch_length(buf, start);
    Ok(())
}

/// Encodes a [`WipMessage`] as a length-prefixed frame appended to `buf`.
pub fn encode_wip(msg: &WipMessage, buf: &mut bytes::BytesMut) -> Result<(), CodecError> {
    let start = buf.len();
    buf.put_u32_le(0); // placeholder; patched below
    match msg {
        WipMessage::PointerMotion(m) => write_pointer_motion(m, buf),
        WipMessage::PointerButton(m) => write_pointer_button(m, buf),
        WipMessage::PointerAxis(m) => write_pointer_axis(m, buf),
        WipMessage::TouchDown(m) => write_touch_point(0x04, m, buf),
        WipMessage::TouchMotion(m) => write_touch_point(0x05, m, buf),
        WipMessage::TouchUp { id, time_ms } => {
            buf.put_u8(0x06);
            buf.put_u32_le(*id);
            buf.put_u32_le(*time_ms);
        }
        WipMessage::StylusDown(m) => write_stylus(0x07, m, buf),
        WipMessage::StylusMotion(m) => write_stylus(0x08, m, buf),
        WipMessage::StylusUp { time_ms } => {
            buf.put_u8(0x09);
            buf.put_u32_le(*time_ms);
        }
        WipMessage::KeyDown(m) => write_key(0x0A, m, buf),
        WipMessage::KeyUp(m) => write_key(0x0B, m, buf),
        WipMessage::Pong { sequence } => {
            buf.put_u8(0x10);
            buf.put_u64_le(*sequence);
        }
    }
    patch_length(buf, start);
    Ok(())
}

/// Patches the 4-byte placeholder at `start` with the actual payload length.
fn patch_length(buf: &mut bytes::BytesMut, start: usize) {
    let payload_len = (buf.len() - start - 4) as u32;
    buf[start..start + 4].copy_from_slice(&payload_len.to_le_bytes());
}

fn write_frame_full(m: &FrameFullMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(0x01);
    buf.put_u32_le(m.width);
    buf.put_u32_le(m.height);
    buf.put_u8(u8::from(m.encoding));
    buf.put_u32_le(m.data.len() as u32);
    buf.put_slice(&m.data);
}

fn write_frame_damage(m: &FrameDamageMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(0x02);
    buf.put_u8(u8::from(m.encoding));
    buf.put_u16_le(m.regions.len() as u16);
    for r in &m.regions {
        write_damage_region(r, buf);
    }
    buf.put_u32_le(m.data.len() as u32);
    buf.put_slice(&m.data);
}

fn write_damage_region(r: &DamageRegion, buf: &mut bytes::BytesMut) {
    buf.put_u32_le(r.x);
    buf.put_u32_le(r.y);
    buf.put_u32_le(r.width);
    buf.put_u32_le(r.height);
}

fn write_display_info(m: &DisplayInfoMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(0x03);
    buf.put_u32_le(m.width);
    buf.put_u32_le(m.height);
    buf.put_f32_le(m.scale_factor);
    buf.put_f32_le(m.refresh_hz);
}

fn write_pointer_motion(m: &PointerMotionMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(0x01);
    buf.put_f32_le(m.x);
    buf.put_f32_le(m.y);
    buf.put_u32_le(m.time_ms);
}

fn write_pointer_button(m: &PointerButtonMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(0x02);
    buf.put_u32_le(m.button);
    buf.put_u8(m.state as u8);
    buf.put_u32_le(m.time_ms);
}

fn write_pointer_axis(m: &PointerAxisMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(0x03);
    buf.put_u8(m.axis as u8);
    buf.put_f32_le(m.value);
    buf.put_u32_le(m.time_ms);
}

fn write_touch_point(discriminant: u8, m: &TouchPointMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(discriminant);
    buf.put_u32_le(m.id);
    buf.put_f32_le(m.x);
    buf.put_f32_le(m.y);
    buf.put_u32_le(m.time_ms);
}

fn write_stylus(discriminant: u8, m: &StylusMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(discriminant);
    buf.put_f32_le(m.x);
    buf.put_f32_le(m.y);
    buf.put_f32_le(m.pressure);
    buf.put_f32_le(m.tilt_x);
    buf.put_f32_le(m.tilt_y);
    buf.put_u32_le(m.time_ms);
}

fn write_key(discriminant: u8, m: &KeyMsg, buf: &mut bytes::BytesMut) {
    buf.put_u8(discriminant);
    buf.put_u32_le(m.keycode);
    buf.put_u32_le(m.modifiers);
    buf.put_u32_le(m.time_ms);
}

