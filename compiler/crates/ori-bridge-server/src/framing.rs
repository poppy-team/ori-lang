use std::io::{self, Read, Write};
use thiserror::Error;

pub const PROTOCOL_MAGIC: [u8; 4] = [0x4F, 0x52, 0x49, 0x42]; // "ORIB"
pub const MAX_FRAME_SIZE: usize = 64 * 1024 * 1024; // 64 MiB

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("invalid protocol magic: expected ORIB")]
    InvalidMagic,
    #[error("frame exceeds maximum allowed size ({size} > {max})")]
    FrameTooLarge { size: usize, max: usize },
    #[error("truncated frame: expected {expected} payload bytes, stream ended")]
    Truncated { expected: usize },
    #[error("empty payload rejected")]
    EmptyPayload,
}

/// Reads a single length-prefixed frame from the reader.
pub fn read_frame<R: Read>(reader: &mut R) -> Result<Vec<u8>, FrameError> {
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic)?;
    if magic != PROTOCOL_MAGIC {
        return Err(FrameError::InvalidMagic);
    }

    let mut len_bytes = [0u8; 4];
    reader.read_exact(&mut len_bytes)?;
    let len = u32::from_le_bytes(len_bytes) as usize;

    if len > MAX_FRAME_SIZE {
        return Err(FrameError::FrameTooLarge {
            size: len,
            max: MAX_FRAME_SIZE,
        });
    }

    if len == 0 {
        return Err(FrameError::EmptyPayload);
    }

    let mut payload = vec![0u8; len];
    match reader.read_exact(&mut payload) {
        Ok(()) => Ok(payload),
        Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => {
            Err(FrameError::Truncated { expected: len })
        }
        Err(e) => Err(FrameError::Io(e)),
    }
}

/// Writes a single length-prefixed frame to the writer.
pub fn write_frame<W: Write>(writer: &mut W, payload: &[u8]) -> Result<(), FrameError> {
    if payload.len() > MAX_FRAME_SIZE {
        return Err(FrameError::FrameTooLarge {
            size: payload.len(),
            max: MAX_FRAME_SIZE,
        });
    }

    writer.write_all(&PROTOCOL_MAGIC)?;
    writer.write_all(&(payload.len() as u32).to_le_bytes())?;
    writer.write_all(payload)?;
    writer.flush()?;
    Ok(())
}
