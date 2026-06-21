//! Passthrough decoder for `RawBgra8` frames.

use bytes::Bytes;

/// Returns the raw BGRA8 payload unchanged.
///
/// The clone is a cheap `Bytes` refcount bump, not a buffer copy.
pub(super) fn decode(data: &Bytes) -> Bytes {
    data.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_payload_unchanged() {
        let data = Bytes::from_static(&[1, 2, 3, 4]);
        assert_eq!(decode(&data), data);
    }
}
