use std::convert::TryInto;
use std::mem::size_of;
use std::ops::Range;

use bytes::{BufMut, Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::{Error, Result};

const BUF_SIZE: usize = 1024;
const MAX_BUF_SIZE: usize = BUF_SIZE * 100;
const HEADER_SIZE: usize = 1;

/// Output from `try_msg`, trying to create a framed message.
/// This is either a heartbeat or a framed message.
#[derive(Debug)]
pub enum FrameOutput {
    /// Framed message (excluding the content length and header)
    Bytes(Box<[u8]>),
    /// A heartbeat
    Heartbeat,
}

impl FrameOutput {
    /// Frame a message by setting a header byte and content length
    ///
    /// ```ignore
    /// # use tokio::io::AsyncRead;
    /// use binky::frame::{Frame, FramedMessage};
    ///
    /// # fn run(mut sender: ClientSender) {
    /// let msg = b"hello world";
    /// let payload: FramedMessage = FrameOutput::frame_message(msg);
    /// # }
    /// ```
    pub(crate) fn frame_message(data: &[u8]) -> FramedMessage {
        let (header, size) = match data.len() as u64 {
            i if i <= u8::MAX as u64 => (Header::Small, size_of::<u8>()),
            i if i <= u32::MAX as u64 => (Header::Large, size_of::<u32>()),
            _ => panic!("invalid content length"),
        };

        let mut payload = BytesMut::with_capacity(data.len() + size + size_of::<Header>());
        payload.put_u8(header as u8);

        match header {
            Header::Small => payload.put_u8(data.len() as u8),
            Header::Large => payload.put_u32(data.len() as u32),
            Header::Unset | Header::Heartbeat => unreachable!(),
        }

        payload.put(data);

        FramedMessage(payload.freeze())
    }
}

/// A message that has a header byte and a content length prefix.
#[derive(Debug, Clone)]
pub(crate) struct FramedMessage(pub Bytes);

impl AsRef<[u8]> for FramedMessage {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
#[non_exhaustive]
pub(crate) enum Header {
    /// Header is not set
    Unset = 0,
    /// A small message where the content length fits inside a single byte
    Small = 1,
    /// A message with a content length as a u32
    Large = 2,
    /// A heartbeat
    Heartbeat = 42,
}

impl Header {
    const fn from_u8(val: u8) -> Option<Self> {
        match val {
            0 => Some(Header::Unset),
            1 => Some(Header::Small),
            2 => Some(Header::Large),
            42 => Some(Header::Heartbeat),
            _ => None,
        }
    }
}

/// The `Frame` is used to frame messages,
/// meaning multiple messages could be delivered in one payload,
/// and the `Frame` is able to separate these messages.
///
/// Messages are separated by a header byte and a content length.
///
/// Example of a small frame with a total
/// length of seven bytes.
///
/// ```text
/// -------------------------------------
/// | Header byte | Size byte | Payload |
/// -------------------------------------
/// | 1           | 7         | .....   |
/// -------------------------------------
/// ```
///
/// Example of a large frame
/// (u32::MAX is the maxiumum payload size):
///
/// ```text
/// ------------------------------------------------
/// | Header byte | Size bytes (4 bytes) | Payload |
/// ------------------------------------------------
/// | 2           | [255, 255, 255, 255] |         |
/// ------------------------------------------------
/// ```
///
///
/// ```ignore
/// # use tokio::io::AsyncRead;
/// use binky::frame::Frame;
/// # async fn run(mut reader: impl AsyncRead + Unpin) {
///
/// let mut frame = Frame::empty();
/// frame.read_async(&mut reader).await.expect("failed to read");
/// match frame.try_msg() {
///     Ok(Some(payload)) => { /* a framed message */ }
///     Ok(None) => { /* read was successful, but didn't contain a complete message */ }
///     Err(e) => { /* error */ }
/// }
/// # }
/// ```
#[derive(Debug)]
pub(crate) struct Frame {
    buffer: Vec<u8>,
    bytes_read: usize,
}

impl Frame {
    /// Create an empty frame, for reading into.
    /// Both `read_async` and `read` will read into the available
    /// space in the buffer. If the buffer runs out of space it will double
    /// until it reaches `MAX_BUF_SIZE`.
    ///
    /// NOTE that reading will only grow the buffer after the currently allocated
    ///      buffer has been consumed. This means that if there is but one byte
    ///      remaining, and the incoming message is two bytes, this still 
    ///      requires two reads to resize the buffer (since it's not possible
    ///      to know how much data is going to be read in total)
    pub(crate) fn empty() -> Self {
        let buffer = Vec::with_capacity(BUF_SIZE);

        Self {
            buffer,
            bytes_read: 0,
        }
    }

    /// Async read
    pub(crate) async fn read_async<T: AsyncRead + Unpin>(&mut self, reader: &mut T) -> Result<usize> {
        let slice = self.available_slice_mut();
        let bytes_read = reader.read(slice).await?;
        self.inner_read(bytes_read)
    }

    /// Sync read
    pub(crate) fn read<T: std::io::Read>(&mut self, reader: &mut T) -> Result<usize> {
        let slice = self.available_slice_mut();
        let bytes_read = reader.read(slice)?;
        self.inner_read(bytes_read)
    }

    // Resize the buffer if needed to fit more data
    fn inner_read(&mut self, bytes_read: usize) -> Result<usize> {
        if bytes_read == 0 {
            return Ok(0);
        }

        self.bytes_read += bytes_read;
        if self.bytes_read < BUF_SIZE && self.buffer.capacity() > BUF_SIZE {
            self.buffer.resize(BUF_SIZE, 0);
        }

        Ok(bytes_read)
    }

    fn extend(&mut self, bytes: &[u8]) -> usize {
        // As long as there is room in the buffer, keep extending the slice
        // until either:
        // * All bytes are consumed
        // * There is no more room in the buffer, and the buffer can not grow.
        let slice = self.available_slice_mut();
        let len = slice.len().min(bytes.len());
        slice[..len].copy_from_slice(&bytes[..len]);
        self.bytes_read += len;
        len
    }

    /// Try to produce a message.
    /// In the event of an incomplete message `Ok(None)` is returned,
    /// and `try_msg` can be called again at a later stage.
    pub fn try_msg(&mut self) -> Result<Option<FrameOutput>> {
        if self.bytes_read == 0 {
            return Ok(None);
        }

        let header = match Header::from_u8(self.buffer[0]) {
            Some(Header::Heartbeat) => {
                // Since it's a heartbeat, just back the bytes_read up by one,
                // so it's overwritten next time, as a `read` and `async_read` will
                // read into `self.available_slice_mut()`.
                self.bytes_read -= 1;
                return Ok(Some(FrameOutput::Heartbeat));
            }
            Some(h) => h,
            None => return Err(Error::MalformedHeader),
        };

        let range = match self.range(header) {
            Some(range) => range,
            None => return Ok(None),
        };

        // This will happen if the message isn't fully read
        if range.end > self.bytes_read {
            return Ok(None);
        }

        let bytes = self.buffer[range.clone()].into();

        self.shift_down_to_next_message(range.end);

        // Shrink the buffer down to `BUF_SIZE` if it has exceeded the `BUF_SIZE`
        // so the buffer doesn't continuously grow
        if self.bytes_read <= BUF_SIZE && self.buffer.capacity() > BUF_SIZE {
            self.buffer.resize(BUF_SIZE, 0);
        }

        Ok(Some(FrameOutput::Bytes(bytes)))
    }

    fn available_slice_mut(&mut self) -> &mut [u8] {
        let slice = &mut self.buffer[self.bytes_read..];
        if slice.is_empty() && self.buffer.capacity() < MAX_BUF_SIZE {
            // Resize the buffer and initiliase it with zeroes
            self.buffer.resize(self.buffer.len() + BUF_SIZE, 0);
        }

        &mut self.buffer[self.bytes_read..]
    }

    fn range(&self, header: Header) -> Option<Range<usize>> {
        match header {
            Header::Small if self.bytes_read >= size_of::<u8>() + HEADER_SIZE => {
                let offset = size_of::<u8>() + HEADER_SIZE;
                let size = self.buffer[1] as usize;
                Some(offset..size + offset)
            }
            Header::Large if self.bytes_read >= size_of::<u32>() + HEADER_SIZE => {
                let offset = HEADER_SIZE + size_of::<u32>();
                let length_bytes: [u8; size_of::<u32>()] = self.buffer[HEADER_SIZE..offset]
                    .try_into()
                    .expect("invalid content length");
                let size = u32::from_be_bytes(length_bytes) as usize;
                Some(offset..size + offset)
            }
            Header::Large | Header::Small => None,
            Header::Unset | Header::Heartbeat => unreachable!(),
        }
    }

    fn shift_down_to_next_message(&mut self, end: usize) {
        // If we read 1 + 1/2 message
        // then we have to shift down the 1/2 message after we extract the bytes
        // from the first read message

        self.buffer.copy_within(end..self.bytes_read, 0);
        self.bytes_read -= end;
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::io::{Read, Result as IoResult};

    struct PretendStream(Vec<u8>);

    impl Read for PretendStream {
        fn read(&mut self, bytes: &mut [u8]) -> IoResult<usize> {
            let bytes_len = bytes.len();
            let self_len = self.0.len();
            let end = bytes.len().min(self.0.len());
            let vec: Vec<u8> = self.0.drain(..end).collect();
            bytes[..end].copy_from_slice(&vec);
            Ok(end)
        }
    }

    #[test]
    fn frame_message() {
        let data = FrameOutput::frame_message(&vec![1u8, 2, 3]);
        let mut stream = PretendStream(data.0.to_vec());
        let mut f = Frame::empty();

        f.read(&mut stream);
        let m = f.try_msg().unwrap().unwrap();
        let FrameOutput::Bytes(msg) = m else { panic!() };
        assert_eq!([1u8, 2, 3].as_slice(), &*msg);
    }

    #[test]
    fn frame_multiple() {
        let mut data = Vec::new();
        let message_count = 3u8;
        // Add three messages to the fake stream
        for i in 0..message_count {
            let framed_msg = FrameOutput::frame_message(&vec![i]);
            data.extend_from_slice(&framed_msg.0);
        }

        let mut stream = PretendStream(data);
        let mut f = Frame::empty();
        f.read(&mut stream);

        // Make sure we can get all messages out
        for i in 0..message_count {
            let m = f.try_msg().unwrap().unwrap();
            let FrameOutput::Bytes(msg) = m else { panic!() };
            assert_eq!([i].as_slice(), &*msg);
        }

        // Make sure there are no residual data
        let m = f.try_msg();
        assert!(m.unwrap().is_none());
    }

    #[test]
    fn auto_grow_buffer() {
        let message = vec![1; BUF_SIZE * 2 + 1];
        let data = FrameOutput::frame_message(&message).0.to_vec();
        let message_len = data.len();
        let mut stream = PretendStream(data);
        let mut f = Frame::empty();

        let n_bytes = f.read(&mut stream).unwrap();
        assert_eq!(n_bytes, BUF_SIZE);

        let _ = f.read(&mut stream).unwrap();
        assert_eq!(f.buffer.len(), BUF_SIZE * 2);

        let _ = f.read(&mut stream).unwrap();
        assert_eq!(f.buffer.len(), BUF_SIZE * 3);

        assert!(f.try_msg().unwrap().is_some());
    }

    #[test]
    fn auto_shrink_buffer() {
        // This produces a message size of BUF_SIZE + 5, where
        // where five is the header and content length
        let data = FrameOutput::frame_message(&vec![1; BUF_SIZE]);

        let mut stream = PretendStream(data.0.to_vec());
        let mut f = Frame::empty();

        assert_eq!(f.read(&mut stream).unwrap(), BUF_SIZE); // read max buf size

        // Reading again casuses the resize as message has not yet been extracted
        assert_eq!(f.read(&mut stream).unwrap(), 5);        // read the last five bytes
        assert_eq!(f.buffer.len(), BUF_SIZE * 2);

        // By taking the message the buffer no longer need to occupy as much
        // space and can be resized.
        f.try_msg().unwrap().unwrap();

        assert_eq!(f.buffer.len(), BUF_SIZE);
    }
}
