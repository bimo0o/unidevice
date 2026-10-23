use crate::error::{Error, Result};
use crate::transport::Transport;

#[derive(Default)]
struct Firewall {
    pending: Option<Vec<u8>>,
    pinged: bool,
    feature: Option<u8>,
    values: Vec<u16>,
    current: Option<u16>,
    setter_used: bool,
    setter_target: Option<u16>,
    faulted: bool,
    basic_absent: bool,
    extended: bool,
    extended_guard: crate::dpi2202::Guard,
}

impl Firewall {
    fn blocked() -> Error {
        Error::Safety(
            "command is outside the documented DPI transaction; reconnect explicitly".into(),
        )
    }

    fn authorize(&mut self, bytes: &[u8]) -> Result<()> {
        if self.faulted
            || self.pending.is_some()
            || !matches!(
                (bytes.first(), bytes.len()),
                (Some(0x10), 7) | (Some(0x11), 20)
            )
            || bytes[1] != 0xff
            || bytes[3] & 15 == 0
        {
            return Err(Self::blocked());
        }
        let function = bytes[3] >> 4;
        let params = &bytes[4..];
        let allowed = if bytes[2] == 0 && bytes.len() == 7 {
            match function {
                1 => !self.pinged && params == [0, 0, 0x5a],
                0 => {
                    self.pinged
                        && self.feature.is_none()
                        && if self.basic_absent {
                            params == [0x22, 2, 0]
                        } else {
                            params == [0x22, 1, 0]
                        }
                }
                _ => false,
            }
        } else if Some(bytes[2]) == self.feature && self.extended {
            self.extended_guard
                .authorize(function, params, bytes.len() == 20)
                .is_ok()
        } else if Some(bytes[2]) == self.feature && bytes.len() == 7 {
            match function {
                0..=2 => params == [0, 0, 0] && (!self.setter_used || function == 2),
                3 => {
                    let dpi = u16::from_be_bytes([bytes[5], bytes[6]]);
                    bytes[4] == 0
                        && !self.setter_used
                        && self.current.is_some()
                        && (100..=25600).contains(&dpi)
                        && self.values.contains(&dpi)
                }
                _ => false,
            }
        } else {
            false
        };
        if !allowed {
            return Err(Self::blocked());
        }
        if !self.extended && bytes[2] != 0 && function == 3 {
            self.setter_used = true;
            self.setter_target = Some(u16::from_be_bytes([bytes[5], bytes[6]]));
        }
        self.pending = Some(bytes.to_vec());
        Ok(())
    }

    fn observe(&mut self, bytes: &[u8]) -> Result<()> {
        let Some(request) = self.pending.clone() else {
            return Err(Self::blocked());
        };
        if bytes.len() < 2 || bytes[1] != 0xff || !matches!(bytes[0], 0x10 | 0x11) {
            return Ok(());
        }
        let report = crate::hidpp::normalized(bytes)?;
        if self.extended && Some(report[2]) == self.feature && matches!(report[3], 0 | 0x10) {
            return Err(Error::Safety(
                "extended DPI changed during transaction; reprobe explicitly".into(),
            ));
        }
        if matches!(report[2], 0xff | 0x8f) {
            if report[3] == request[2] && report[4] == request[3] {
                self.faulted = true;
                self.pending = None;
            }
            return Ok(());
        }
        if report[2] != request[2] || report[3] != request[3] {
            return Ok(());
        }
        self.pending = None;
        let data = &report[4..];
        if self.extended && Some(request[2]) == self.feature {
            return self
                .extended_guard
                .observe(request[3] >> 4, &request[4..], data);
        }
        match (request[2], request[3] >> 4) {
            (0, 1) if data[0] >= 2 && data[2] == 0x5a => self.pinged = true,
            (0, 0) if data[0] == 0 && request[4..] == [0x22, 1, 0] => self.basic_absent = true,
            (0, 0) if (1..0xf0).contains(&data[0]) && data[0] != 0x8f && data[1] & 0xc0 == 0 => {
                self.extended = request[4..] == [0x22, 2, 0];
                if self.extended && data[2] != 0 {
                    return Err(Self::blocked());
                }
                self.feature = Some(data[0]);
            }
            (feature, 1) if Some(feature) == self.feature && data.len() == 16 && data[0] == 0 => {
                self.values = crate::hidpp::parse_dpi_list(&data[1..])?;
            }
            (feature, 2) if Some(feature) == self.feature && data[0] == 0 => {
                let value = u16::from_be_bytes([data[1], data[2]]);
                if value == 0 || value >= 0xe000 {
                    return Err(Self::blocked());
                }
                self.current = Some(value);
                if self.setter_used {
                    if self.setter_target != Some(value) {
                        return Err(Self::blocked());
                    }
                    self.setter_used = false;
                    self.setter_target = None;
                    self.current = None;
                    self.values.clear();
                }
            }
            (feature, 0 | 3) if Some(feature) == self.feature => {}
            _ => return Err(Self::blocked()),
        }
        Ok(())
    }
}

pub struct GuardedTransport<T> {
    inner: T,
    firewall: Box<Firewall>,
}
impl<T> GuardedTransport<T> {
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            firewall: Box::default(),
        }
    }
    #[cfg(test)]
    pub(crate) fn inner(&self) -> &T {
        &self.inner
    }
}
impl<T: Transport> Transport for GuardedTransport<T> {
    fn write(&mut self, report: &[u8]) -> Result<()> {
        if let Err(error) = self.firewall.authorize(report) {
            self.firewall.faulted = true;
            return Err(error);
        }
        if let Err(error) = self.inner.write(report) {
            self.firewall.faulted = true;
            return Err(error);
        }
        Ok(())
    }
    fn read(&mut self, timeout_ms: i32) -> Result<Vec<u8>> {
        let result = self.inner.read(timeout_ms).and_then(|reply| {
            self.firewall.observe(&reply)?;
            Ok(reply)
        });
        if result.is_err() {
            self.firewall.faulted = true;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::ScriptedTransport;

    #[test]
    fn unknown_reset_flash_and_wrong_route_never_reach_device() {
        for packet in [
            vec![7, 2, 0xaa, 0, 0, 0, 0],
            vec![0x10, 1, 0, 0x11, 0, 0, 0x5a],
            vec![0x10, 0xff, 0, 1, 0, 0xc0, 0],
            vec![0x10, 0xff, 7, 0x31, 0, 3, 0x20],
            vec![0x11; 20],
            vec![0; 64],
            vec![],
        ] {
            let mut transport = GuardedTransport::new(ScriptedTransport::default());
            assert!(matches!(transport.write(&packet), Err(Error::Safety(_))));
            assert!(transport.inner().writes.is_empty());
        }
    }

    #[test]
    fn only_one_validated_setter_is_permitted_per_handle() {
        let mut guard = Firewall {
            pinged: true,
            feature: Some(7),
            values: vec![400, 800],
            current: Some(400),
            ..Default::default()
        };
        for dpi in [0u16, 100, 801, 65535] {
            let [hi, lo] = dpi.to_be_bytes();
            assert!(guard.authorize(&[0x10, 0xff, 7, 0x31, 0, hi, lo]).is_err());
        }
        for feature in [0, 1, 8, 0x8f, 0xff] {
            assert!(guard
                .authorize(&[0x10, 0xff, feature, 0x31, 0, 3, 0x20])
                .is_err());
        }
        for function in 4..16 {
            assert!(guard
                .authorize(&[0x10, 0xff, 7, (function << 4) | 1, 0, 0, 0])
                .is_err());
        }
        guard.authorize(&[0x10, 0xff, 7, 0x31, 0, 3, 0x20]).unwrap();
        guard.observe(&[0x10, 0xff, 7, 0x31, 0, 3, 0x20]).unwrap();
        assert!(guard.authorize(&[0x10, 0xff, 7, 0x32, 0, 1, 0x90]).is_err());
    }

    #[test]
    fn setter_requires_both_current_read_and_capabilities() {
        let mut guard = Firewall {
            pinged: true,
            feature: Some(7),
            ..Default::default()
        };
        let set = [0x10, 0xff, 7, 0x31, 0, 3, 0x20];
        assert!(guard.authorize(&set).is_err());
        guard.values = vec![800];
        assert!(guard.authorize(&set).is_err());
        guard.current = Some(400);
        assert!(guard.authorize(&set).is_ok());
    }

    #[test]
    fn timeout_latches_transport_without_retry() {
        let mut transport = GuardedTransport::new(ScriptedTransport::default());
        let ping = [0x10, 0xff, 0, 0x11, 0, 0, 0x5a];
        transport.write(&ping).unwrap();
        assert_eq!(transport.read(1), Err(Error::Timeout));
        assert!(transport.write(&ping).is_err());
        assert_eq!(transport.inner().writes.len(), 1);
    }

    #[test]
    fn complete_transaction_passes_both_protocol_and_firewall() {
        let mock = crate::hidpp::demo_session().unwrap().into_transport();
        let mut initial = crate::transport::ScriptedTransport::default();
        initial.replies.extend([
            Ok(vec![0x10, 0xff, 0, 0x11, 2, 0, 0x5a]),
            Ok(vec![0x10, 0xff, 0, 2, 7, 0, 1]),
            Ok(vec![0x10, 0xff, 7, 3, 1, 0, 0]),
        ]);
        initial.replies.extend(mock.replies);
        let mut session = crate::hidpp::Session::connect(GuardedTransport::new(initial)).unwrap();
        assert_eq!(session.set_dpi(800), Ok(800));
        let transport = session.into_transport();
        assert_eq!(
            transport
                .inner()
                .writes
                .iter()
                .filter(|p| p[2] == 7 && p[3] >> 4 == 3)
                .count(),
            1
        );
        assert!(transport.firewall.values.is_empty());
    }
}
