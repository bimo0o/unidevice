use crate::error::{Error, Result};
use hidapi::HidDevice;
use std::collections::VecDeque;

pub trait Transport {
    fn write(&mut self, report: &[u8]) -> Result<()>;
    fn read(&mut self, timeout_ms: i32) -> Result<Vec<u8>>;
}

pub struct NativeTransport(crate::safety::GuardedTransport<NativeIo>);

impl NativeTransport {
    pub fn new(device: HidDevice) -> Result<Self> {
        let descriptor = crate::device::read_descriptor(&device)?;
        if !descriptor.hidpp_framing() {
            return Err(Error::Safety("HID++ descriptor framing required".into()));
        }
        let extended = descriptor.hidpp_extended_framing();
        Ok(Self(crate::safety::GuardedTransport::new(NativeIo {
            device,
            extended,
        })))
    }
}

impl Transport for NativeTransport {
    fn write(&mut self, report: &[u8]) -> Result<()> {
        self.0.write(report)
    }
    fn read(&mut self, timeout_ms: i32) -> Result<Vec<u8>> {
        self.0.read(timeout_ms)
    }
}

struct NativeIo {
    device: HidDevice,
    extended: bool,
}

impl Transport for NativeIo {
    fn write(&mut self, report: &[u8]) -> Result<()> {
        if !self.extended
            && (report.first() == Some(&0x11) || report.get(4..) == Some(&[0x22, 2, 0]))
        {
            return Err(Error::Safety(
                "extended DPI needs descriptor input/output long reports".into(),
            ));
        }
        let len = self.device.write(report).map_err(|_| Error::Disconnected)?;
        if len != report.len() {
            return Err(Error::ShortWrite);
        }
        Ok(())
    }

    fn read(&mut self, timeout_ms: i32) -> Result<Vec<u8>> {
        let mut buffer = [0; 64];
        let len = self
            .device
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
