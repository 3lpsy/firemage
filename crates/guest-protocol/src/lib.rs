use std::io::{self, Read, Write};

use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const PORT: u32 = 1024;
pub const VERSION: u32 = 1;
pub const MAX_FRAME_BYTES: usize = 65_536;
pub const MAX_DATA_BYTES: usize = 16_384;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ClientMessage {
    Open {
        version: u32,
        command: Vec<String>,
        rows: u16,
        cols: u16,
    },
    Input {
        data: String,
    },
    Resize {
        rows: u16,
        cols: u16,
    },
    Close,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ServerMessage {
    Ready,
    Output { data: String },
    Exit { code: Option<i32> },
    Error { message: String },
}

pub fn validate_size(rows: u16, cols: u16) -> io::Result<()> {
    if !(1..=1000).contains(&rows) || !(1..=1000).contains(&cols) {
        return Err(invalid("terminal dimensions must be between 1 and 1000"));
    }
    Ok(())
}

pub fn validate_open(version: u32, command: &[String], rows: u16, cols: u16) -> io::Result<()> {
    if version != VERSION {
        return Err(invalid("unsupported guest protocol version"));
    }
    validate_size(rows, cols)?;
    if command.len() > 64
        || command.iter().map(String::len).sum::<usize>() > 16_384
        || command
            .iter()
            .any(|arg| arg.len() > 4096 || arg.contains('\0'))
        || command.first().is_some_and(String::is_empty)
    {
        return Err(invalid("invalid shell command"));
    }
    Ok(())
}

pub fn encode_data(data: &[u8]) -> String {
    STANDARD.encode(data)
}

pub fn decode_data(data: &str) -> io::Result<Vec<u8>> {
    if data.len() > MAX_DATA_BYTES.div_ceil(3) * 4 {
        return Err(invalid("terminal input too large"));
    }
    let decoded = STANDARD
        .decode(data)
        .map_err(|_| invalid("invalid terminal input encoding"))?;
    if decoded.len() > MAX_DATA_BYTES {
        return Err(invalid("terminal input too large"));
    }
    Ok(decoded)
}

pub fn read_frame<R: Read, T: DeserializeOwned>(reader: &mut R) -> io::Result<T> {
    let mut header = [0; 4];
    reader.read_exact(&mut header)?;
    let size = u32::from_be_bytes(header) as usize;
    if size == 0 || size > MAX_FRAME_BYTES {
        return Err(invalid("invalid guest frame length"));
    }
    let mut data = vec![0; size];
    reader.read_exact(&mut data)?;
    serde_json::from_slice(&data).map_err(|_| invalid("invalid guest frame"))
}

pub fn write_frame<W: Write, T: Serialize>(writer: &mut W, message: &T) -> io::Result<()> {
    let data = serde_json::to_vec(message).map_err(io::Error::other)?;
    if data.is_empty() || data.len() > MAX_FRAME_BYTES {
        return Err(invalid("invalid guest frame length"));
    }
    writer.write_all(&(data.len() as u32).to_be_bytes())?;
    writer.write_all(&data)
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests;
