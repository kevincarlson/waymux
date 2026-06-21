//! Length-prefixed framing and the primitive byte helpers shared by the
//! [`WfpMessage`](crate::WfpMessage) and [`WipMessage`](crate::WipMessage)
//! body codecs.
//!
//! Wire framing: every message is encoded as a little-endian `u32` payload
//! length followed by exactly that many payload bytes. The payload itself
//! begins with a one-byte message-type discriminant.

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::CodecError;
use crate::wfp::WfpMessage;
use crate::wip::WipMessage;

/// Number of bytes in the length prefix that precedes every frame body.
const LEN_PREFIX: usize = 4;

/// Encodes `msg` as a length-prefixed WFP frame appended to `buf`.
///
/// # Errors
/// Returns an error if the message body exceeds the `u32` framing length. On
/// error `buf` is rolled back to its previous length.
pub fn encode_wfp(msg: &WfpMessage, buf: &mut BytesMut) -> Result<(), CodecError> {
    encode_framed(buf, |body| msg.encode(body))
}

/// Encodes `msg` as a length-prefixed WIP frame appended to `buf`.
///
/// # Errors
/// See [`encode_wfp`].
pub fn encode_wip(msg: &WipMessage, buf: &mut BytesMut) -> Result<(), CodecError> {
    encode_framed(buf, |body| msg.encode(body))
}

/// Attempts to decode one WFP message from the front of `buf`.
///
/// Returns `Ok(None)` when more bytes are required to complete the frame,
/// leaving `buf` untouched. A fully received frame is removed from `buf`
/// before its body is parsed.
///
/// # Errors
/// Returns an error if a complete frame body is malformed or carries an
/// unknown discriminant.
pub fn decode_wfp(buf: &mut BytesMut) -> Result<Option<WfpMessage>, CodecError> {
    decode_framed(buf, WfpMessage::decode)
}

/// Attempts to decode one WIP message from the front of `buf`.
///
/// See [`decode_wfp`] for the framing semantics.
///
/// # Errors
/// Returns an error if a complete frame body is malformed or carries an
/// unknown discriminant.
pub fn decode_wip(buf: &mut BytesMut) -> Result<Option<WipMessage>, CodecError> {
    decode_framed(buf, WipMessage::decode)
}

/// Writes a length placeholder, runs `body`, then backfills the real length.
///
/// Backfilling avoids a second allocation per frame: the body is serialised
/// directly into the output buffer rather than into a temporary.
fn encode_framed<F>(buf: &mut BytesMut, body: F) -> Result<(), CodecError>
where
    F: FnOnce(&mut BytesMut) -> Result<(), CodecError>,
{
    let start = buf.len();
    buf.put_u32_le(0);
    if let Err(err) = body(buf) {
        buf.truncate(start);
        return Err(err);
    }
    let body_len = buf.len() - start - LEN_PREFIX;
    match u32::try_from(body_len) {
        Ok(len) => {
            buf[start..start + LEN_PREFIX].copy_from_slice(&len.to_le_bytes());
            Ok(())
        }
        Err(_) => {
            buf.truncate(start);
            Err(CodecError::PayloadTooLarge(body_len))
        }
    }
}

/// Splits one complete frame off the front of `buf` and parses it with `body`.
fn decode_framed<T, F>(buf: &mut BytesMut, body: F) -> Result<Option<T>, CodecError>
where
    F: FnOnce(&mut Bytes) -> Result<T, CodecError>,
{
    if buf.len() < LEN_PREFIX {
        return Ok(None);
    }
    let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() - LEN_PREFIX < len {
        return Ok(None);
    }
    buf.advance(LEN_PREFIX);
    let mut payload = buf.split_to(len).freeze();
    let msg = body(&mut payload)?;
    if payload.has_remaining() {
        return Err(CodecError::Malformed("trailing bytes after frame body"));
    }
    Ok(Some(msg))
}

// --- primitive readers shared by the `wfp` and `wip` body decoders ---

pub(crate) fn read_u8(buf: &mut Bytes) -> Result<u8, CodecError> {
    ensure(buf, 1)?;
    Ok(buf.get_u8())
}

pub(crate) fn read_u16(buf: &mut Bytes) -> Result<u16, CodecError> {
    ensure(buf, 2)?;
    Ok(buf.get_u16_le())
}

pub(crate) fn read_u32(buf: &mut Bytes) -> Result<u32, CodecError> {
    ensure(buf, 4)?;
    Ok(buf.get_u32_le())
}

pub(crate) fn read_u64(buf: &mut Bytes) -> Result<u64, CodecError> {
    ensure(buf, 8)?;
    Ok(buf.get_u64_le())
}

pub(crate) fn read_f32(buf: &mut Bytes) -> Result<f32, CodecError> {
    ensure(buf, 4)?;
    Ok(buf.get_f32_le())
}

/// Reads a `u32` length prefix and then that many payload bytes (zero-copy).
pub(crate) fn read_len_prefixed(buf: &mut Bytes) -> Result<Bytes, CodecError> {
    let len = read_u32(buf)? as usize;
    ensure(buf, len)?;
    Ok(buf.split_to(len))
}

/// Writes `data` preceded by its `u32` little-endian length.
pub(crate) fn put_len_prefixed(buf: &mut BytesMut, data: &[u8]) -> Result<(), CodecError> {
    let len = u32::try_from(data.len()).map_err(|_| CodecError::PayloadTooLarge(data.len()))?;
    buf.put_u32_le(len);
    buf.put_slice(data);
    Ok(())
}

fn ensure(buf: &Bytes, needed: usize) -> Result<(), CodecError> {
    if buf.remaining() < needed {
        return Err(CodecError::Malformed("frame body ended prematurely"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wfp::{DisplayInfoMsg, WfpMessage};

    fn sample() -> WfpMessage {
        WfpMessage::DisplayInfo(DisplayInfoMsg {
            width: 1920,
            height: 1080,
            scale_factor: 2.0,
            refresh_hz: 60.0,
        })
    }

    #[test]
    fn empty_buffer_decodes_to_none() {
        let mut buf = BytesMut::new();
        assert_eq!(decode_wfp(&mut buf), Ok(None));
    }

    #[test]
    fn partial_prefix_decodes_to_none_without_consuming() {
        let mut buf = BytesMut::new();
        buf.put_u8(0x01);
        assert_eq!(decode_wfp(&mut buf), Ok(None));
        assert_eq!(buf.len(), 1, "partial frame must not be consumed");
    }

    #[test]
    fn two_frames_decode_in_order() {
        let mut buf = BytesMut::new();
        encode_wfp(&sample(), &mut buf).expect("encode 1");
        encode_wfp(&WfpMessage::Ping { sequence: 7 }, &mut buf).expect("encode 2");

        assert_eq!(decode_wfp(&mut buf), Ok(Some(sample())));
        assert_eq!(
            decode_wfp(&mut buf),
            Ok(Some(WfpMessage::Ping { sequence: 7 }))
        );
        assert_eq!(decode_wfp(&mut buf), Ok(None));
        assert!(buf.is_empty());
    }
}
