//! Decoder for `ZstdBgra8` frames, using the pure-Rust `ruzstd` decompressor.
//!
//! The client only ever decompresses, so a decode-only pure-Rust implementation
//! keeps the crate free of C dependencies and trivially cross-compilable to
//! Android.

use std::io::Read;

use bytes::Bytes;

use crate::error::ClientError;

/// Decompresses a zstd-compressed BGRA8 payload.
///
/// # Errors
/// Returns [`ClientError::Decompress`] if the payload is not valid zstd.
pub(super) fn decode(data: &Bytes) -> Result<Bytes, ClientError> {
    let mut decoder = ruzstd::StreamingDecoder::new(data.as_ref())
        .map_err(|err| ClientError::Decompress(err.to_string()))?;
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|err| ClientError::Decompress(err.to_string()))?;
    Ok(Bytes::from(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_zstd_payload() {
        let original = vec![0x5au8; 1024];
        let compressed = zstd::encode_all(original.as_slice(), 3).unwrap();
        let decoded = decode(&Bytes::from(compressed)).unwrap();
        assert_eq!(decoded.as_ref(), original.as_slice());
    }

    #[test]
    fn rejects_invalid_payload() {
        let result = decode(&Bytes::from_static(&[0xff, 0x00, 0x12, 0x34]));
        assert!(matches!(result, Err(ClientError::Decompress(_))));
    }
}
