//! Per-client session: a drop-oldest outbound frame queue plus the read/write
//! tasks that service one connected client.

use std::collections::VecDeque;
use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{Mutex, Notify, mpsc};
use tracing::trace;
use waymux_proto::{WfpMessage, WipMessage, decode_wip, encode_wfp};

use crate::error::BridgeError;

/// Bounded outbound queue that drops the **oldest** frame on overflow.
///
/// Dropping the oldest (rather than the newest) keeps latency low: a client
/// that briefly stalls resumes on the most recent frame instead of replaying
/// stale ones.
#[derive(Debug)]
pub struct FrameQueue {
    inner: Mutex<VecDeque<Bytes>>,
    notify: Notify,
    capacity: usize,
}

impl FrameQueue {
    /// Creates a queue holding at most `capacity` frames.
    #[must_use]
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(VecDeque::with_capacity(capacity.max(1))),
            notify: Notify::new(),
            capacity: capacity.max(1),
        })
    }

    /// Enqueues `frame`, evicting the oldest frame if the queue is full.
    pub async fn push(&self, frame: Bytes) {
        {
            let mut queue = self.inner.lock().await;
            if queue.len() >= self.capacity {
                queue.pop_front();
            }
            queue.push_back(frame);
        }
        self.notify.notify_one();
    }

    /// Waits for and removes the next frame.
    pub async fn pop(&self) -> Bytes {
        loop {
            // Register for notification before checking, so a push that races
            // with this check is not missed.
            let notified = self.notify.notified();
            if let Some(frame) = self.inner.lock().await.pop_front() {
                return frame;
            }
            notified.await;
        }
    }
}

/// Drives one client connection until either side closes.
///
/// Sends `DisplayInfo` first, then streams queued frames; concurrently reads
/// inbound WIP messages, forwarding them to `input` (the injector) when present.
pub async fn run(
    stream: UnixStream,
    queue: Arc<FrameQueue>,
    display_info: WfpMessage,
    input: Option<mpsc::Sender<WipMessage>>,
) -> Result<(), BridgeError> {
    let (mut read_half, mut write_half) = stream.into_split();

    let mut hello = BytesMut::new();
    encode_wfp(&display_info, &mut hello)?;
    write_half.write_all(&hello).await?;

    let writer = async {
        loop {
            let frame = queue.pop().await;
            write_half.write_all(&frame).await?;
        }
        // unreachable Ok used only for type inference
        #[allow(unreachable_code)]
        Ok::<(), BridgeError>(())
    };

    tokio::select! {
        result = writer => result,
        result = drain_input(&mut read_half, input.as_ref()) => result,
    }
}

/// Reads inbound WIP messages, forwarding to `input` until the client closes.
async fn drain_input(
    read_half: &mut tokio::net::unix::OwnedReadHalf,
    input: Option<&mpsc::Sender<WipMessage>>,
) -> Result<(), BridgeError> {
    let mut buf = BytesMut::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        // Decode any complete messages already buffered.
        while let Some(msg) = decode_wip(&mut buf)? {
            match input {
                // Drop under backpressure: freshest input matters most.
                Some(sink) => {
                    let _ = sink.try_send(msg);
                }
                None => trace!(?msg, "no injector configured; dropping input"),
            }
        }
        let read = read_half.read(&mut chunk).await?;
        if read == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn push_drops_oldest_when_full() {
        let queue = FrameQueue::new(2);
        queue.push(Bytes::from_static(b"a")).await;
        queue.push(Bytes::from_static(b"b")).await;
        queue.push(Bytes::from_static(b"c")).await; // evicts "a"

        assert_eq!(queue.pop().await, Bytes::from_static(b"b"));
        assert_eq!(queue.pop().await, Bytes::from_static(b"c"));
    }

    #[tokio::test]
    async fn pop_waits_for_a_push() {
        let queue = FrameQueue::new(4);
        let waiter = queue.clone();
        let handle = tokio::spawn(async move { waiter.pop().await });
        queue.push(Bytes::from_static(b"z")).await;
        assert_eq!(handle.await.unwrap(), Bytes::from_static(b"z"));
    }
}
