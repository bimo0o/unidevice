use crate::error::{Error, Result};
use crate::transport::Transport;
use std::time::{Duration, Instant};

const DEVICE: u8 = 0xff;
const DEADLINE: Duration = Duration::from_millis(800);

pub fn short_request(feature: u8, function: u8, software: u8, params: [u8; 3]) -> Vec<u8> {
    vec![
        0x10,
        DEVICE,
        feature,
        (function << 4) | software,
        params[0],
        params[1],
        params[2],
    ]
}

fn normalized(report: &[u8]) -> Result<&[u8]> {
    let expected = match report.first() {
        Some(0x10) => 7,
        Some(0x11) => 20,
        _ => return Err(Error::Protocol("unknown report ID".into())),
    };
    if report.len() < expected || report.len() > 64 || report[expected..].iter().any(|b| *b != 0) {
        return Err(Error::Protocol("report length or padding".into()));
    }
    Ok(&report[..expected])
}

pub struct Session<T> {
    transport: T,
    software: u8,
    feature: u8,
}

impl<T: Transport> Session<T> {
    pub fn connect(transport: T) -> Result<Self> {
        let mut session = Self {
            transport,
            software: 0,
            feature: 0,
        };
        let ping = session.exchange(0, 1, [0, 0, 0x5a])?;
        if ping[0] < 2 || ping[2] != 0x5a {
            return Err(Error::Protocol("HID++2 ping version or echo".into()));
        }
        let feature = session.exchange(0, 0, [0x22, 0x01, 0])?;
        if feature[0] == 0 {
            return Err(Error::Unsupported(
                "device has no Adjustable DPI (0x2201); no fallback writes".into(),
            ));
        }
        if feature[1] & 0xc0 != 0 {
            return Err(Error::Unsupported(
                "DPI feature is obsolete or hidden".into(),
            ));
        }
        session.feature = feature[0];
        let sensors = session.exchange(session.feature, 0, [0, 0, 0])?;
        if sensors[0] == 0 {
            return Err(Error::Protocol("device reports no sensors".into()));
        }
        Ok(session)
    }

    fn exchange(&mut self, feature: u8, function: u8, params: [u8; 3]) -> Result<Vec<u8>> {
        self.software = self.software % 15 + 1;
        let request = short_request(feature, function, self.software, params);
        self.transport.write(&request)?;
        let start = Instant::now();
        for _ in 0..64 {
            let remaining = DEADLINE
                .checked_sub(start.elapsed())
                .ok_or(Error::Timeout)?;
            let reply = self.transport.read(remaining.as_millis().max(1) as i32)?;
            if reply.len() < 2 {
                return Err(Error::Protocol("truncated report header".into()));
            }
            if reply[1] != DEVICE || !matches!(reply[0], 0x10 | 0x11) {
                continue;
            }
            let reply = normalized(&reply)?;
            if matches!(reply[2], 0xff | 0x8f) {
                if reply[3] == request[2] && reply[4] == request[3] {
                    return Err(Error::Rejected(reply[5]));
                }
                continue;
            }
            if reply[2] != request[2] || reply[3] != request[3] {
                continue;
            }
            return Ok(reply[4..].to_vec());
        }
        Err(Error::Timeout)
    }

    pub fn supported_dpi(&mut self) -> Result<Vec<u16>> {
        let data = self.exchange(self.feature, 1, [0, 0, 0])?;
        if data.len() != 16 || data[0] != 0 {
            return Err(Error::Protocol(
                "DPI list must be a long response for sensor 0".into(),
            ));
        }
        parse_dpi_list(&data[1..])
    }

    pub fn read_dpi(&mut self) -> Result<u16> {
        let data = self.exchange(self.feature, 2, [0, 0, 0])?;
        if data[0] != 0 {
            return Err(Error::Protocol("DPI response sensor mismatch".into()));
        }
        let dpi = u16::from_be_bytes([data[1], data[2]]);
        if dpi == 0 || dpi >= 0xe000 {
            return Err(Error::Protocol("invalid current DPI".into()));
        }
        Ok(dpi)
    }

    pub fn set_dpi(&mut self, dpi: u16) -> Result<u16> {
        if !(100..=25600).contains(&dpi) {
            return Err(Error::Invalid(
                "experimental DPI safety range is 100..25600".into(),
            ));
        }
        if !self.supported_dpi()?.contains(&dpi) {
            return Err(Error::Unsupported(
                "requested DPI is not in device capabilities".into(),
            ));
        }
        let [hi, lo] = dpi.to_be_bytes();
        self.exchange(self.feature, 3, [0, hi, lo])?;
        let actual = self.read_dpi()?;
        if actual != dpi {
            return Err(Error::Protocol(format!(
                "DPI readback is {actual}, expected {dpi}; state may have changed"
            )));
        }
        Ok(actual)
    }

    pub fn into_transport(self) -> T {
        self.transport
    }
}

pub fn parse_dpi_list(data: &[u8]) -> Result<Vec<u16>> {
    let words: Vec<u16> = data
        .chunks_exact(2)
        .map(|b| u16::from_be_bytes([b[0], b[1]]))
        .collect();
    let mut result = Vec::new();
    let mut i = 0;
    while i < words.len() && words[i] != 0 {
        let value = words[i];
        if value >= 0xe000 {
            return Err(Error::Protocol("DPI range marker without start".into()));
        }
        if result.last().is_some_and(|last| *last >= value) {
            return Err(Error::Protocol("unordered DPI list".into()));
        }
        if i + 1 < words.len() && words[i + 1] >= 0xe000 {
            let step = words[i + 1] & 0x1fff;
            let end = *words
                .get(i + 2)
                .ok_or_else(|| Error::Protocol("truncated DPI range".into()))?;
            if step == 0 || end >= 0xe000 || end < value || (end - value) % step != 0 {
                return Err(Error::Protocol("invalid DPI range bounds or step".into()));
            }
            result.extend((value..=end).step_by(step as usize));
            i += 3;
        } else {
            result.push(value);
            i += 1;
        }
    }
    if result.is_empty() {
        return Err(Error::Protocol("empty DPI capabilities".into()));
    }
    if words[i..].iter().any(|w| *w != 0) {
        return Err(Error::Protocol("data after DPI list terminator".into()));
    }
    Ok(result)
}

pub fn demo_session() -> Result<Session<crate::transport::ScriptedTransport>> {
    use crate::transport::ScriptedTransport;
    let requests = vec![
        short_request(0, 1, 1, [0, 0, 0x5a]),
        short_request(0, 0, 2, [0x22, 0x01, 0]),
        short_request(7, 0, 3, [0, 0, 0]),
        short_request(7, 1, 4, [0, 0, 0]),
        short_request(7, 3, 5, [0, 3, 0x20]),
        short_request(7, 2, 6, [0, 0, 0]),
    ];
    let mut capabilities = vec![0x11, DEVICE, 7, 0x14, 0, 0x01, 0x90, 0xe0, 0x32, 0x64, 0];
    capabilities.resize(20, 0);
    let replies = vec![
        Ok(vec![0x10, DEVICE, 0, 0x11, 2, 0, 0x5a]),
        Ok(vec![0x10, DEVICE, 0, 2, 7, 0, 1]),
        Ok(vec![0x10, DEVICE, 7, 3, 1, 0, 0]),
        Ok(capabilities),
        Ok(vec![0x10, DEVICE, 7, 0x35, 0, 3, 0x20]),
        Ok(vec![0x10, DEVICE, 7, 0x26, 0, 3, 0x20]),
    ];
    Session::connect(ScriptedTransport {
        expected: requests.into(),
        replies: replies.into(),
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::ScriptedTransport;

    #[test]
    fn complete_dpi_transaction_matches_independent_fixture() {
        let mut session = demo_session().unwrap();
        assert_eq!(session.set_dpi(800), Ok(800));
        let transport = session.into_transport();
        assert!(transport.expected.is_empty());
        assert!(transport.replies.is_empty());
        assert_eq!(
            transport.writes.last().unwrap(),
            &[0x10, 0xff, 7, 0x26, 0, 0, 0]
        );
    }

    #[test]
    fn dpi_lists_and_ranges() {
        assert_eq!(
            parse_dpi_list(&[1, 0x90, 3, 0x20, 6, 0x40, 0, 0]),
            Ok(vec![400, 800, 1600])
        );
        assert_eq!(
            parse_dpi_list(&[1, 0x90, 0xe0, 0xc8, 3, 0x20]),
            Ok(vec![400, 600, 800])
        );
    }

    #[test]
    fn malformed_capabilities_are_rejected() {
        for data in [
            vec![0xe0, 0],
            vec![1, 0x90, 0xe0, 0, 3, 0x20],
            vec![1, 0x90, 0xe0, 0xc8],
            vec![3, 0x20, 0xe0, 0xc8, 1, 0x90],
            vec![0, 0],
            vec![1, 0x90, 1, 0x90],
            vec![1, 0x90, 0, 0, 3, 0x20],
        ] {
            assert!(parse_dpi_list(&data).is_err(), "{data:?}");
        }
    }

    #[test]
    fn timeout_disconnect_and_short_write_propagate() {
        for error in [Error::Timeout, Error::Disconnected, Error::ShortWrite] {
            let transport = ScriptedTransport {
                write_error: Some(error.clone()),
                ..Default::default()
            };
            assert!(matches!(Session::connect(transport), Err(e) if e == error));
        }
        assert!(matches!(
            Session::connect(ScriptedTransport::default()),
            Err(Error::Timeout)
        ));
    }

    #[test]
    fn rejection_checks_original_request() {
        for error_id in [0xff, 0x8f] {
            let transport = ScriptedTransport {
                replies: vec![Ok(vec![0x10, DEVICE, error_id, 0, 0x11, 8, 0])].into(),
                ..Default::default()
            };
            assert!(matches!(
                Session::connect(transport),
                Err(Error::Rejected(8))
            ));
        }
    }

    #[test]
    fn malformed_reports_never_panic() {
        for reply in [
            vec![],
            vec![0x10],
            vec![0x10, DEVICE],
            vec![0x11, DEVICE, 0, 0x11, 2, 0, 0x5a],
        ] {
            let transport = ScriptedTransport {
                replies: vec![Ok(reply)].into(),
                ..Default::default()
            };
            assert!(matches!(
                Session::connect(transport),
                Err(Error::Protocol(_))
            ));
        }
    }

    #[test]
    fn wrong_report_or_correlation_cannot_complete_request() {
        for reply in [
            vec![0x12, DEVICE, 0, 0x11, 2, 0, 0x5a],
            vec![0x10, 1, 0, 0x11, 2, 0, 0x5a],
            vec![0x10, DEVICE, 9, 0x11, 2, 0, 0x5a],
            vec![0x10, DEVICE, 0, 0x12, 2, 0, 0x5a],
            vec![0x10, DEVICE, 0xff, 9, 0x11, 8, 0],
        ] {
            let transport = ScriptedTransport {
                replies: vec![Ok(reply)].into(),
                ..Default::default()
            };
            assert!(matches!(Session::connect(transport), Err(Error::Timeout)));
        }
    }

    #[test]
    fn absent_feature_and_bad_ping_block_control() {
        let mut session = demo_session().unwrap();
        session.feature = 0;
        let mut transport = session.into_transport();
        transport.expected.clear();
        transport.replies = vec![
            Ok(vec![0x10, DEVICE, 0, 0x11, 2, 0, 0x5a]),
            Ok(vec![0x10, DEVICE, 0, 2, 0, 0, 0]),
        ]
        .into();
        assert!(matches!(
            Session::connect(transport),
            Err(Error::Unsupported(_))
        ));
        let transport = ScriptedTransport {
            replies: vec![Ok(vec![0x10, DEVICE, 0, 0x11, 1, 0, 0x5a])].into(),
            ..Default::default()
        };
        assert!(matches!(
            Session::connect(transport),
            Err(Error::Protocol(_))
        ));
    }

    #[test]
    fn unsupported_dpi_never_reaches_setter() {
        let mut session = demo_session().unwrap();
        assert!(matches!(session.set_dpi(801), Err(Error::Unsupported(_))));
        assert_eq!(session.into_transport().writes.len(), 4);
    }

    #[test]
    fn failed_readback_is_not_success() {
        let mut session = demo_session().unwrap();
        *session.transport.replies.back_mut().unwrap() =
            Ok(vec![0x10, DEVICE, 7, 0x26, 0, 6, 0x40]);
        assert!(matches!(session.set_dpi(800), Err(Error::Protocol(_))));
    }

    #[test]
    fn zero_padding_is_allowed_but_data_is_not() {
        let mut report = vec![0x10, DEVICE, 0, 0x11, 2, 0, 0x5a];
        report.resize(20, 0);
        assert_eq!(normalized(&report).unwrap().len(), 7);
        report[19] = 1;
        assert!(normalized(&report).is_err());
    }

    #[test]
    fn previously_unlisted_device_can_report_a_different_feature_index() {
        let transport = ScriptedTransport {
            replies: vec![
                Ok(vec![0x10, DEVICE, 0, 0x11, 2, 0, 0x5a]),
                Ok(vec![0x10, DEVICE, 0, 2, 42, 0, 1]),
                Ok(vec![0x10, DEVICE, 42, 3, 1, 0, 0]),
                Ok(vec![0x10, DEVICE, 42, 0x24, 0, 3, 0x20]),
            ]
            .into(),
            ..Default::default()
        };
        let mut session = Session::connect(transport).unwrap();
        assert_eq!(session.read_dpi(), Ok(800));
        assert_eq!(
            session.into_transport().writes[3],
            [0x10, DEVICE, 42, 0x24, 0, 0, 0]
        );
    }
}
