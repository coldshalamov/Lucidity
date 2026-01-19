use thiserror::Error;

pub const MAX_FRAME_LEN: u32 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub typ: u8,
    pub payload: Vec<u8>,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DecodeError {
    #[error("buffer too short")]
    BufferTooShort,
    #[error("declared length {0} exceeds max {MAX_FRAME_LEN}")]
    LengthTooLarge(u32),
    #[error("declared length {0} is invalid")]
    InvalidLength(u32),
}

impl Frame {
    pub fn encode_to_vec(&self) -> Vec<u8> {
        encode_frame(self.typ, &self.payload)
    }
}

pub fn encode_frame(typ: u8, payload: &[u8]) -> Vec<u8> {
    let len = payload.len() + 1;
    let len_u32: u32 = len
        .try_into()
        .unwrap_or_else(|_| panic!("payload too large: {} bytes", payload.len()));
    let mut out = Vec::with_capacity(4 + len);
    out.extend_from_slice(&len_u32.to_le_bytes());
    out.push(typ);
    out.extend_from_slice(payload);
    out
}

#[derive(Debug, Default)]
pub struct FrameDecoder {
    buf: Vec<u8>,
    read_idx: usize,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self { buf: Vec::new(), read_idx: 0 }
    }

    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn next_frame(&mut self) -> Result<Option<Frame>, DecodeError> {
        let available = self.buf.len() - self.read_idx;
        if available < 4 {
            return Ok(None);
        }

        let len_bytes = &self.buf[self.read_idx..self.read_idx + 4];
        let len = u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]);

        if len > MAX_FRAME_LEN {
            return Err(DecodeError::LengthTooLarge(len));
        }
        if len == 0 {
            return Err(DecodeError::InvalidLength(len));
        }

        let total = 4usize + (len as usize);
        if available < total {
            return Ok(None);
        }

        let typ_idx = self.read_idx + 4;
        let typ = self.buf[typ_idx];
        let payload_start = typ_idx + 1;
        let payload_end = self.read_idx + total;
        
        let payload = self.buf[payload_start..payload_end].to_vec();

        self.read_idx += total;
        
        // If we've reached the end, clear the buffer to reclaim memory
        if self.read_idx == self.buf.len() {
            self.buf.clear();
            self.read_idx = 0;
        } else if self.read_idx > 64 * 1024 {
            // If the buffer is getting large and we've read a lot, compact it
            self.buf.drain(0..self.read_idx);
            self.read_idx = 0;
        }

        Ok(Some(Frame { typ, payload }))
    }

    pub fn take_buffered_len(&self) -> usize {
        self.buf.len() - self.read_idx
    }
}
