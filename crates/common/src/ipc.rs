use std::io::{self, Error, ErrorKind};
use serde::{de::DeserializeOwned, Serialize};

/// Maximum allowed IPC frame size (1 MB) to prevent denial of service
pub const MAX_IPC_FRAME_SIZE: usize = 1024 * 1024;

/// Encodes a message into length-prefixed bytes (4-byte big-endian length + JSON)
pub fn encode_frame<T: Serialize>(msg: &T) -> io::Result<Vec<u8>> {
    let json_bytes = serde_json::to_vec(msg)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    
    if json_bytes.len() > MAX_IPC_FRAME_SIZE {
        return Err(Error::new(ErrorKind::InvalidData, "Payload exceeds maximum frame size"));
    }

    let len = json_bytes.len() as u32;
    let mut frame = Vec::with_capacity(4 + json_bytes.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(&json_bytes);
    Ok(frame)
}

/// Decodes a length-prefixed frame from raw bytes
pub fn decode_frame<T: DeserializeOwned>(bytes: &[u8]) -> io::Result<T> {
    if bytes.len() < 4 {
        return Err(Error::new(ErrorKind::UnexpectedEof, "Frame too short"));
    }
    let len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    if bytes.len() < 4 + len {
        return Err(Error::new(ErrorKind::UnexpectedEof, "Incomplete frame"));
    }
    let payload = &bytes[4..4 + len];
    serde_json::from_slice(payload)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))
}
