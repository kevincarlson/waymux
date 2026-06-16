//! Unix-socket transport: reads framed WFP messages from the bridge and writes
//! framed WIP messages back.

use std::path::Path;

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::net::unix::{OwnedReadHalf, OwnedWriteHalf};
use waymux_proto::{WfpMessage, WipMessage, decode_wfp, encode_wip};

use crate::error::ClientError;

/// Read buffer growth chunk size.
const CHUNK: usize = 16 * 1024;

/// Connects to the bridge socket at `path`, returning split read/write ends.
///
/// # Errors
/// Returns [`ClientError::Io`] if the socket cannot be reached.
pub async fn connect(path: &Path) -> Result<(Reader, Writer), ClientError> {
    let stream = UnixStream::connect(path).await?;
    let (read_half, write_half) = stream.into_split();
    Ok((
        Reader {
            half: read_half,
            buf: BytesMut::with_capacity(CHUNK),
        },
        Writer { half: write_half },
    ))
}

/// The read end of a bridge connection.
pub struct Reader {
    half: OwnedReadHalf,
    buf: BytesMut,
}

impl Reader {
    /// Reads the next WFP message, returning `None` on a clean end of stream.
    ///
    /// # Errors
    /// Returns [`ClientError::Codec`] on a malformed frame, [`ClientError::Closed`]
    /// if the stream ends mid-frame, or [`ClientError::Io`] on a socket error.
    pub async fn next(&mut self) -> Result<Option<WfpMessage>, ClientError> {
        loop {
            if let Some(msg) = decode_wfp(&mut self.buf)? {
                return Ok(Some(msg));
            }
            let mut chunk = [0u8; CHUNK];
            let read = self.half.read(&mut chunk).await?;
            if read == 0 {
                return if self.buf.is_empty() {
                    Ok(None)
                } else {
                    Err(ClientError::Closed)
                };
            }
            self.buf.extend_from_slice(&chunk[..read]);
        }
    }
}

/// The write end of a bridge connection.
pub struct Writer {
    half: OwnedWriteHalf,
}

impl Writer {
    /// Serializes and sends one WIP message.
    ///
    /// # Errors
    /// Returns [`ClientError::Codec`] if encoding fails or [`ClientError::Io`]
    /// on a socket error.
    pub async fn send(&mut self, msg: &WipMessage) -> Result<(), ClientError> {
        let mut buf = BytesMut::new();
        encode_wip(msg, &mut buf)?;
        self.half.write_all(&buf).await?;
        Ok(())
    }
}
