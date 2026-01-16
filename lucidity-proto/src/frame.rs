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
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self { buf: Vec::new() }
    }

    pub fn push(&mut self, data: &[u8]) {
        self.buf.extend_from_slice(data);
    }

    pub fn next_frame(&mut self) -> Result<Option<Frame>, DecodeError> {
        if self.buf.len() < 4 {
            return Ok(None);
        }

        let len = u32::from_le_bytes([
            self.buf[0],
            self.buf[1],
            self.buf[2],
            self.buf[3],
        ]);

        if len > MAX_FRAME_LEN {
            return Err(DecodeError::LengthTooLarge(len));
        }
        if len == 0 {
            return Err(DecodeError::InvalidLength(len));
        }

        let total = 4usize + (len as usize);
        if self.buf.len() < total {
            return Ok(None);
        }

        let typ = self.buf[4];
        let payload = self.buf[(4 + 1)..total].to_vec();

        self.buf.drain(0..total);
        Ok(Some(Frame { typ, payload }))
    }

    pub fn take_buffered_len(&self) -> usize {
        self.buf.len()
    }
}

