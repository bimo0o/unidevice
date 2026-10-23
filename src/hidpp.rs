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

pub(crate) fn normalized(report: &[u8]) -> Result<&[u8]> {
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
    extended_flags: Option<u8>,
    rgb_checked: bool,
    rgb_layout: Option<crate::rgb8070::Layout>,
    faulted: bool,
}

impl<T: Transport> Session<T> {
    pub fn connect(transport: T) -> Result<Self> {
        let mut session = Self {
            transport,
            software: 0,
            feature: 0,
            extended_flags: None,
            rgb_checked: false,
            rgb_layout: None,
            faulted: false,
        };
        let ping = session.exchange(0, 1, [0, 0, 0x5a])?;
        if ping[0] < 2 || ping[2] != 0x5a {
            return Err(Error::Protocol("HID++2 ping version or echo".into()));
        }
        let mut feature = session.exchange(0, 0, [0x22, 0x01, 0])?;
        if feature[0] == 0 {
            feature = session.exchange(0, 0, [0x22, 0x02, 0])?;
            if feature[2] != 0 {
                return Err(Error::Unsupported(
                    "unreviewed extended DPI feature version".into(),
                ));
            }
            session.extended_flags = Some(0);
        }
        if feature[0] == 0 || feature[0] >= 0xf0 || feature[0] == 0x8f {
            return Err(Error::Unsupported(
                "device has no reviewed Adjustable DPI (0x2201/0x2202); no fallback writes".into(),
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
        if session.extended_flags.is_some() {
            session.extended_capabilities()?;
        }
        Ok(session)
    }

    fn exchange(&mut self, feature: u8, function: u8, params: [u8; 3]) -> Result<Vec<u8>> {
        self.exchange_payload(feature, function, &params)
    }

    fn exchange_payload(&mut self, feature: u8, function: u8, params: &[u8]) -> Result<Vec<u8>> {
        if self.faulted {
            return Err(Error::Safety(
                "session faulted; probe again explicitly".into(),
            ));
        }
        self.software = self.software % 15 + 1;
        let mut request = vec![
            if params.len() == 16 { 0x11 } else { 0x10 },
            DEVICE,
            feature,
            (function << 4) | self.software,
        ];
        request.extend_from_slice(params);
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
        if self.extended_flags.is_some() {
            let flags = self.extended_capabilities()?;
            let mut axes = Vec::new();
            for direction in 0..=u8::from(flags & 1 != 0) {
                let mut pages = crate::dpi2202::Pages::default();
                while let Some(page) = pages.next() {
                    let reply = self.exchange(self.feature, 2, [0, direction, page])?;
                    pages.accept(direction, page, &reply)?;
                }
                axes.push(
                    pages.values.ok_or_else(|| {
                        Error::Protocol("unterminated extended DPI ranges".into())
                    })?,
                );
            }
            let mut values = axes.remove(0);
            if let Some(y) = axes.first() {
                values.retain(|v| y.binary_search(v).is_ok());
            }
            if values.is_empty() {
                return Err(Error::Unsupported("no shared X/Y DPI capabilities".into()));
            }
            return Ok(values);
        }
        let data = self.exchange(self.feature, 1, [0, 0, 0])?;
        if data.len() != 16 || data[0] != 0 {
            return Err(Error::Protocol(
                "DPI list must be a long response for sensor 0".into(),
            ));
        }
        parse_dpi_list(&data[1..])
    }

    pub fn read_dpi(&mut self) -> Result<u16> {
        if self.extended_flags.is_some() {
            return self.extended_parameters()?.scalar();
        }
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
        let supported = self.supported_dpi()?;
        if !supported.contains(&dpi) {
            return Err(Error::Unsupported(
                "requested DPI is not in device capabilities".into(),
            ));
        }
        if self.extended_flags.is_some() {
            let before = self.extended_parameters()?;
            if !supported.contains(&before.scalar()?) || !(100..=25600).contains(&before.x) {
                return Err(Error::Safety(
                    "current extended DPI is outside restorable capabilities".into(),
                ));
            }
            let payload = before.setter(dpi)?;
            if before.scalar()? == dpi {
                return Ok(dpi);
            }
            let ack = self.exchange_payload(self.feature, 6, &payload)?;
            if ack.first() != Some(&0) {
                return Err(Error::Protocol(
                    "extended DPI setter sensor mismatch".into(),
                ));
            }
            let after = self.extended_parameters()?;
            if after.setter(dpi)? != payload
                || after.scalar()? != dpi
                || !before.same_defaults(after)
            {
                return Err(Error::Protocol(
                    "extended DPI readback changed DPI, LOD or defaults; no retry".into(),
                ));
            }
            return Ok(dpi);
        }
        if self.read_dpi()? == dpi {
            return Ok(dpi);
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

    pub fn protocol_id(&self) -> u16 {
        if self.extended_flags.is_some() {
            0x2202
        } else {
            0x2201
        }
    }

    pub fn discover_rgb(&mut self) -> Result<bool> {
        if self.rgb_checked {
            return Ok(self.rgb_layout.is_some());
        }
        self.rgb_checked = true;
        if !self.transport.supports_long() {
            return Ok(false);
        }
        let feature = self.exchange(0, 0, [0x80, 0x70, 0])?;
        if feature[0] == 0 || feature[1] & 0xc0 != 0 || feature[2] > 7 {
            return Ok(false);
        }
        if feature[0] >= 0xf0 || feature[0] == 0x8f || feature[0] == self.feature {
            return Err(Error::Protocol(
                "invalid or overlapping RGB feature index".into(),
            ));
        }
        let index = feature[0];
        let info = self.exchange(index, 0, [0, 0, 0])?;
        let count = match crate::rgb8070::zone_count(&info) {
            Ok(n) => n,
            Err(Error::Unsupported(_)) => return Ok(false),
            Err(e) => return Err(e),
        };
        let mut zones = Vec::new();
        for zone in 0..count {
            let data = self.exchange(index, 1, [zone, 0, 0])?;
            let mut z = crate::rgb8070::Zone::info(zone, &data)?;
            for effect in 0..z.effects.len() as u8 {
                let data = self.exchange(index, 2, [zone, effect, 0])?;
                z.effect_info(effect, &data)?;
            }
            zones.push(z);
        }
        let layout = crate::rgb8070::Layout { index, zones };
        if let Err(Error::Unsupported(_)) = layout.validate() {
            return Ok(false);
        }
        layout.validate()?;
        self.rgb_layout = Some(layout);
        Ok(true)
    }
    pub fn disable_rgb(&mut self) {
        self.rgb_layout = None;
    }
    pub fn rgb_snapshot(&mut self) -> Result<crate::rgb8070::Snapshot> {
        let layout = self
            .rgb_layout
            .clone()
            .ok_or_else(|| Error::Unsupported("no restorable RGB8070 layout".into()))?;
        let mut settings = Vec::new();
        for zone in &layout.zones {
            let reply = self.exchange(layout.index, 14, [zone.index, 0, 0])?;
            settings.push(zone.read(&reply)?);
        }
        Ok(crate::rgb8070::Snapshot { settings })
    }
    pub fn set_rgb(&mut self, color: [u8; 3]) -> Result<crate::rgb8070::Snapshot> {
        let layout = self
            .rgb_layout
            .as_ref()
            .ok_or_else(|| Error::Unsupported("no RGB8070 layout".into()))?;
        let settings = layout
            .zones
            .iter()
            .map(|zone| zone.fixed(color))
            .collect::<Result<Vec<_>>>()?;
        self.restore_rgb(&crate::rgb8070::Snapshot { settings })
    }
    pub fn restore_rgb(
        &mut self,
        target: &crate::rgb8070::Snapshot,
    ) -> Result<crate::rgb8070::Snapshot> {
        let result = self.restore_rgb_inner(target);
        if result.is_err() {
            self.faulted = true;
        }
        result
    }
    fn restore_rgb_inner(
        &mut self,
        target: &crate::rgb8070::Snapshot,
    ) -> Result<crate::rgb8070::Snapshot> {
        let layout = self
            .rgb_layout
            .clone()
            .ok_or_else(|| Error::Unsupported("no RGB8070 layout".into()))?;
        if target.settings.len() != layout.zones.len() {
            return Err(Error::Safety("RGB snapshot layout changed".into()));
        }
        // Validate all targets before the first setter; imported files cannot construct these settings.
        for (zone, setting) in layout.zones.iter().zip(&target.settings) {
            if setting.zone_index() != zone.index {
                return Err(Error::Safety("RGB snapshot zone order changed".into()));
            }
            layout.payload(setting)?;
        }
        let dpi = self.read_dpi()?;
        let before = self.rgb_snapshot()?;
        for (setting, previous) in target.settings.iter().zip(&before.settings) {
            if setting == previous {
                continue;
            }
            let payload = layout.payload(setting)?;
            self.exchange_payload(layout.index, 3, &payload)?;
            let actual = self.exchange(layout.index, 14, [setting.zone_index(), 0, 0])?;
            if layout.zones[usize::from(setting.zone_index())].read(&actual)? != *setting {
                return Err(Error::Protocol(
                    "RGB8070 readback mismatch; no rollback or retry".into(),
                ));
            }
        }
        if self.read_dpi()? != dpi {
            return Err(Error::Safety(
                "DPI changed during RGB update; stop control".into(),
            ));
        }
        Ok(target.clone())
    }
    fn extended_capabilities(&mut self) -> Result<u8> {
        let data = self.exchange(self.feature, 1, [0, 0, 0])?;
        if data[0] != 0 || data[2] & !15 != 0 {
            return Err(Error::Protocol(
                "extended DPI capability sensor or flags".into(),
            ));
        }
        self.extended_flags = Some(data[2]);
        Ok(data[2])
    }
    fn extended_parameters(&mut self) -> Result<crate::dpi2202::Parameters> {
        let data = self.exchange(self.feature, 5, [0, 0, 0])?;
        crate::dpi2202::Parameters::parse(&data, self.extended_flags.ok_or(Error::NotFound)?)
    }
}

pub fn parse_dpi_list(data: &[u8]) -> Result<Vec<u16>> {
    let words: Vec<u16> = data
        .as_chunks::<2>()
        .0
        .iter()
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
            if step == 0 || end >= 0xe000 || end < value || !(end - value).is_multiple_of(step) {
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
        short_request(7, 2, 5, [0, 0, 0]),
        short_request(7, 3, 6, [0, 3, 0x20]),
        short_request(7, 2, 7, [0, 0, 0]),
    ];
    let mut capabilities = vec![0x11, DEVICE, 7, 0x14, 0, 0x01, 0x90, 0xe0, 0x32, 0x64, 0];
    capabilities.resize(20, 0);
    let replies = vec![
        Ok(vec![0x10, DEVICE, 0, 0x11, 2, 0, 0x5a]),
        Ok(vec![0x10, DEVICE, 0, 2, 7, 0, 1]),
        Ok(vec![0x10, DEVICE, 7, 3, 1, 0, 0]),
        Ok(capabilities),
        Ok(vec![0x10, DEVICE, 7, 0x25, 0, 1, 0x90]),
        Ok(vec![0x10, DEVICE, 7, 0x36, 0, 3, 0x20]),
        Ok(vec![0x10, DEVICE, 7, 0x27, 0, 3, 0x20]),
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
            &[0x10, 0xff, 7, 0x27, 0, 0, 0]
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
            Ok(vec![0x10, DEVICE, 0, 3, 0, 0, 0]),
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
            Ok(vec![0x10, DEVICE, 7, 0x27, 0, 6, 0x40]);
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
