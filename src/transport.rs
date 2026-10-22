use crate::error::{Error, Result};
use hidapi::HidDevice;
use std::collections::VecDeque;

pub trait Transport {
    fn write(&mut self, report: &[u8]) -> Result<()>;
    fn read(&mut self, timeout_ms: i32) -> Result<Vec<u8>>;
}

pub struct NativeTransport(pub HidDevice);

impl Transport for NativeTransport {
    fn write(&mut self, report: &[u8]) -> Result<()> {
        let len = self.0.write(report).map_err(|_| Error::Disconnected)?;
        if len != report.len() {
            return Err(Error::ShortWrite);
        }
        Ok(())
    }

    fn read(&mut self, timeout_ms: i32) -> Result<Vec<u8>> {
        let mut buffer = [0; 64];
        let len = self
            .0
            .read_timeout(&mut buffer, timeout_ms)
            .map_err(|_| Error::Disconnected)?;
        if len == 0 {
            return Err(Error::Timeout);
        }
        Ok(buffer[..len].to_vec())
    }
}

#[derive(Default)]
pub struct ScriptedTransport {
    pub expected: VecDeque<Vec<u8>>,
    pub replies: VecDeque<Result<Vec<u8>>>,
    pub writes: Vec<Vec<u8>>,
    pub write_error: Option<Error>,
}

impl Transport for ScriptedTransport {
    fn write(&mut self, report: &[u8]) -> Result<()> {
        if let Some(e) = &self.write_error {
            return Err(e.clone());
        }
        if let Some(expected) = self.expected.pop_front() {
            if expected != report {
                return Err(Error::Protocol("mock request differs from fixture".into()));
            }
        }
        self.writes.push(report.to_vec());
        Ok(())
    }

    fn read(&mut self, _: i32) -> Result<Vec<u8>> {
        self.replies.pop_front().unwrap_or(Err(Error::Timeout))
    }
}
