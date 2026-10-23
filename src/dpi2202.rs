//! Independent implementation of the extended DPI wire facts in specs/hidpp2202.md.
//! This module has no HID access. Calibration, profile writes and LED commands do not exist here.
use crate::error::{Error, Result};

fn invalid() -> Error {
    Error::Protocol("invalid extended DPI state, capabilities or range page".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hidpp::Session,
        safety::GuardedTransport,
        transport::{ScriptedTransport, Transport},
    };

    // Independently specified wire fixtures, not device captures or generated requests.
    fn fixtures() -> (Vec<Vec<u8>>, Vec<Result<Vec<u8>>>) {
        let mut setter = vec![0x11, 0xff, 9, 0x6a, 0, 6, 0x40, 6, 0x40, 2];
        setter.resize(20, 0);
        let requests = vec![
            vec![0x10, 0xff, 0, 0x11, 0, 0, 0x5a],
            vec![0x10, 0xff, 0, 2, 0x22, 1, 0],
            vec![0x10, 0xff, 0, 3, 0x22, 2, 0],
            vec![0x10, 0xff, 9, 4, 0, 0, 0],
            vec![0x10, 0xff, 9, 0x15, 0, 0, 0],
            vec![0x10, 0xff, 9, 0x16, 0, 0, 0],
            vec![0x10, 0xff, 9, 0x27, 0, 0, 0],
            vec![0x10, 0xff, 9, 0x28, 0, 1, 0],
            vec![0x10, 0xff, 9, 0x59, 0, 0, 0],
            setter,
            vec![0x10, 0xff, 9, 0x5b, 0, 0, 0],
        ];
        let long = |header: [u8; 4], payload: &[u8]| {
            let mut r = header.to_vec();
            r.extend_from_slice(payload);
            r.resize(20, 0);
            Ok(r)
        };
        let replies = vec![
            Ok(vec![0x10, 0xff, 0, 0x11, 2, 0, 0x5a]),
            Ok(vec![0x10, 0xff, 0, 2, 0, 0, 0]),
            Ok(vec![0x10, 0xff, 0, 3, 9, 0, 0]),
            Ok(vec![0x10, 0xff, 9, 4, 1, 0, 0]),
            Ok(vec![0x10, 0xff, 9, 0x15, 0, 5, 3]),
            Ok(vec![0x10, 0xff, 9, 0x16, 0, 5, 3]),
            long(
                [0x11, 0xff, 9, 0x27],
                &[0, 0, 0, 1, 0x90, 0xe0, 0xc8, 6, 0x40, 0, 0],
            ),
            long(
                [0x11, 0xff, 9, 0x28],
                &[0, 1, 0, 1, 0x90, 0xe0, 0xc8, 6, 0x40, 0, 0],
            ),
            long(
                [0x11, 0xff, 9, 0x59],
                &[0, 3, 0x20, 1, 0x90, 3, 0x20, 1, 0x90, 2],
            ),
            Ok(vec![0x10, 0xff, 9, 0x6a, 0, 0, 0]),
            long(
                [0x11, 0xff, 9, 0x5b],
                &[0, 6, 0x40, 1, 0x90, 6, 0x40, 1, 0x90, 2],
            ),
        ];
        (requests, replies)
    }
    fn transport(replies: Vec<Result<Vec<u8>>>) -> GuardedTransport<ScriptedTransport> {
        GuardedTransport::new(ScriptedTransport {
            expected: fixtures().0.into(),
            replies: replies.into(),
            ..Default::default()
        })
    }
    fn replay(count: usize) -> GuardedTransport<ScriptedTransport> {
        let (requests, replies) = fixtures();
        let mut t = transport(replies);
        for request in requests.iter().take(count) {
            t.write(request).unwrap();
            t.read(10).unwrap();
        }
        t
    }
    #[test]
    fn fallback_and_live_set_preserve_lod_and_defaults_through_native_firewall() {
        let mut session = Session::connect(transport(fixtures().1)).unwrap();
        assert_eq!(session.protocol_id(), 0x2202);
        assert_eq!(session.set_dpi(1600).unwrap(), 1600);
        let t = session.into_transport();
        assert_eq!(t.inner().writes, fixtures().0);
        assert!(t.inner().expected.is_empty());
    }
    #[test]
    fn runtime_index_is_learned_and_second_apply_requires_all_fresh_reads() {
        let (mut requests, mut replies) = fixtures();
        let mut setter = vec![0x11, 0xff, 9, 0x61, 0, 3, 32, 3, 32, 2];
        setter.resize(20, 0);
        let extra = vec![
            vec![0x10, 0xff, 9, 0x1c, 0, 0, 0],
            vec![0x10, 0xff, 9, 0x2d, 0, 0, 0],
            vec![0x10, 0xff, 9, 0x2e, 0, 1, 0],
            vec![0x10, 0xff, 9, 0x5f, 0, 0, 0],
            setter,
            vec![0x10, 0xff, 9, 0x52, 0, 0, 0],
        ];
        for (source, header) in [
            (5, 0x1c),
            (6, 0x2d),
            (7, 0x2e),
            (10, 0x5f),
            (9, 0x61),
            (8, 0x52),
        ] {
            let mut reply = replies[source].clone().unwrap();
            reply[3] = header;
            replies.push(Ok(reply));
        }
        requests.extend(extra);
        for r in &mut requests {
            if r[2] == 9 {
                r[2] = 0x21;
            }
        }
        for r in &mut replies {
            if r.as_ref().unwrap()[2] == 9 {
                r.as_mut().unwrap()[2] = 0x21;
            }
        }
        replies[2].as_mut().unwrap()[4] = 0x21;
        let t = GuardedTransport::new(ScriptedTransport {
            expected: requests.clone().into(),
            replies: replies.into(),
            ..Default::default()
        });
        let mut session = Session::connect(t).unwrap();
        assert_eq!(session.set_dpi(1600).unwrap(), 1600);
        assert_eq!(session.set_dpi(800).unwrap(), 800);
        assert_eq!(session.into_transport().inner().writes, requests);
    }
    #[test]
    fn every_setting_payload_mutation_wrong_route_and_calibration_is_blocked_before_io() {
        for offset in 0..20 {
            let mut t = replay(9);
            let mut changed = fixtures().0[9].clone();
            changed[offset] ^= if offset == 3 { 0x10 } else { 1 };
            assert!(t.write(&changed).is_err(), "byte {offset}");
            assert_eq!(t.inner().writes.len(), 9);
            assert!(t.write(&fixtures().0[9]).is_err());
        }
        for function in [3, 4, 7, 8, 9, 10, 15] {
            let mut t = replay(9);
            let mut changed = fixtures().0[9].clone();
            changed[3] = (function << 4) | 10;
            assert!(t.write(&changed).is_err());
            assert_eq!(t.inner().writes.len(), 9);
        }
    }
    #[test]
    fn readback_mismatch_timeout_and_asymmetric_axes_stop_without_retry() {
        for (reply, offset) in [(10, 5), (10, 9), (10, 13), (10, 7), (8, 9), (6, 5), (4, 4)] {
            let (_, mut replies) = fixtures();
            replies[reply].as_mut().unwrap()[offset] ^= 1;
            let mut session = match Session::connect(transport(replies)) {
                Ok(s) => s,
                Err(_) => {
                    assert!(reply < 5);
                    continue;
                }
            };
            assert!(
                session.set_dpi(1600).is_err(),
                "reply {reply} byte {offset}"
            );
            let t = session.into_transport();
            assert!(t.inner().writes.len() <= if reply < 9 { 9 } else { 11 });
            assert_eq!(
                t.inner().writes.iter().filter(|r| r[3] >> 4 == 6).count(),
                usize::from(reply >= 9)
            );
        }
        for reply in [9, 10] {
            let (_, mut replies) = fixtures();
            replies[reply] = Err(Error::Timeout);
            let mut session = Session::connect(transport(replies)).unwrap();
            assert!(session.set_dpi(1600).is_err());
            let mut t = session.into_transport();
            let count = t.inner().writes.len();
            assert!(t.write(&fixtures().0[9]).is_err());
            assert_eq!(t.inner().writes.len(), count);
        }
    }
    #[test]
    fn range_words_cross_page_boundaries_and_shared_endpoints_are_deduplicated() {
        let mut stream = Vec::new();
        for w in [100u16, 200, 300, 400, 500, 600, 700, 0xe064, 1000, 1000, 0] {
            stream.extend_from_slice(&w.to_be_bytes());
        }
        stream.resize(26, 0);
        let mut pages = Pages::default();
        let mut first = vec![0, 0, 0];
        first.extend_from_slice(&stream[..13]);
        pages.accept(0, 0, &first).unwrap();
        assert_eq!(pages.next(), Some(1));
        let mut second = vec![0, 0, 1];
        second.extend_from_slice(&stream[13..]);
        pages.accept(0, 1, &second).unwrap();
        assert_eq!(
            pages.values.unwrap(),
            (100..=1000).step_by(100).collect::<Vec<_>>()
        );
        for payload in [
            [0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            [0, 0, 0, 1, 0x90, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0],
        ] {
            assert!(Pages::default().accept(0, 0, &payload).is_err());
        }
    }

    #[test]
    fn no_independent_axis_or_lod_never_acquires_invented_values() {
        let (mut requests, mut replies) = fixtures();
        requests.remove(7);
        replies.remove(7).unwrap();
        for i in 7..requests.len() {
            requests[i][3] -= 1;
            replies[i].as_mut().unwrap()[3] -= 1;
        }
        replies[4].as_mut().unwrap()[6] = 0;
        replies[5].as_mut().unwrap()[6] = 0;
        for i in [7, 9] {
            replies[i].as_mut().unwrap()[9..14].fill(0);
        }
        requests[8][7..10].fill(0);
        let native = GuardedTransport::new(ScriptedTransport {
            expected: requests.clone().into(),
            replies: replies.into(),
            ..Default::default()
        });
        let mut session = Session::connect(native).unwrap();
        assert_eq!(session.set_dpi(1600).unwrap(), 1600);
        assert_eq!(session.into_transport().inner().writes, requests);
        let mut payload = [0; 16];
        payload[1..5].copy_from_slice(&[3, 32, 1, 0x90]);
        assert!(Parameters::parse(&payload, 0).is_ok());
        payload[9] = 1;
        assert!(Parameters::parse(&payload, 0).is_err());
        payload[9] = 0;
        payload[5] = 1;
        assert!(Parameters::parse(&payload, 0).is_err());
    }

    #[test]
    fn unterminated_wrong_pages_unknown_flags_and_short_states_are_rejected() {
        let mut pages = Pages::default();
        for page in 0..16 {
            let mut data = [1; 16];
            data[..3].copy_from_slice(&[0, 0, page]);
            assert_eq!(pages.accept(0, page, &data).is_ok(), page < 15);
        }
        for len in 0..16 {
            assert!(Pages::default().accept(0, 0, &vec![0; len]).is_err());
            assert!(Parameters::parse(&vec![0; len], 0).is_err());
        }
        for (reply, offset, value) in [(2, 6, 1), (2, 5, 0x40), (4, 6, 0x80), (6, 6, 1)] {
            let (_, mut replies) = fixtures();
            replies[reply].as_mut().unwrap()[offset] = value;
            match Session::connect(transport(replies)) {
                Ok(mut session) => {
                    assert!(session.set_dpi(1600).is_err());
                    assert_eq!(session.into_transport().inner().writes.len(), 7);
                }
                Err(_) => assert!(reply < 5),
            }
        }
        let (_, mut replies) = fixtures();
        replies[8] = Ok(vec![0x10, 0xff, 9, 0x59, 0, 3, 32]);
        let mut session = Session::connect(transport(replies)).unwrap();
        assert!(session.set_dpi(1600).is_err());
        assert_eq!(session.into_transport().inner().writes.len(), 9);
    }

    #[test]
    fn missing_preconditions_unsupported_dpi_events_and_duplicate_setters_cannot_write() {
        for prefix in [0, 1, 2, 3, 4, 5, 6, 7, 8] {
            let mut t = replay(prefix);
            assert!(t.write(&fixtures().0[9]).is_err());
            assert_eq!(t.inner().writes.len(), prefix);
        }
        let mut session = Session::connect(transport(fixtures().1)).unwrap();
        assert!(session.set_dpi(1700).is_err());
        assert_eq!(session.into_transport().inner().writes.len(), 8);
        let mut t = replay(10);
        assert!(t.write(&fixtures().0[9]).is_err());
        assert_eq!(t.inner().writes.len(), 10);
        let (_, mut replies) = fixtures();
        replies[8] = Ok(vec![0x10, 0xff, 9, 0, 0, 3, 32]);
        let mut session = Session::connect(transport(replies)).unwrap();
        assert!(session.set_dpi(1600).is_err());
        assert_eq!(session.into_transport().inner().writes.len(), 9);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Parameters {
    pub x: u16,
    default_x: u16,
    pub y: u16,
    default_y: u16,
    pub lod: u8,
}
impl Parameters {
    pub fn parse(data: &[u8], flags: u8) -> Result<Self> {
        if data.len() != 16
            || data[0] != 0
            || flags & !15 != 0
            || data[10..].iter().any(|b| *b != 0)
        {
            return Err(invalid());
        }
        let word = |i| u16::from_be_bytes([data[i], data[i + 1]]);
        let p = Self {
            x: word(1),
            default_x: word(3),
            y: word(5),
            default_y: word(7),
            lod: data[9],
        };
        let dpi = |value| (1..0xe000).contains(&value);
        if !dpi(p.x)
            || !dpi(p.default_x)
            || (flags & 1 != 0 && (!dpi(p.y) || !dpi(p.default_y)))
            || (flags & 1 == 0 && (p.y != 0 || p.default_y != 0))
            || (flags & 2 != 0 && !(1..=3).contains(&p.lod))
            || (flags & 2 == 0 && p.lod != 0)
        {
            return Err(invalid());
        }
        Ok(p)
    }
    pub fn scalar(self) -> Result<u16> {
        if self.y != 0 && self.y != self.x {
            return Err(Error::Unsupported(
                "independent X/Y DPI requires an axis-aware UI; settings unchanged".into(),
            ));
        }
        Ok(self.x)
    }
    pub fn same_defaults(self, other: Self) -> bool {
        self.default_x == other.default_x && self.default_y == other.default_y
    }
    pub fn setter(self, dpi: u16) -> Result<[u8; 16]> {
        self.scalar()?;
        if !(100..=25600).contains(&dpi) {
            return Err(Error::Invalid(
                "experimental DPI policy is 100..25600".into(),
            ));
        }
        let mut data = [0; 16];
        data[1..3].copy_from_slice(&dpi.to_be_bytes());
        if self.y != 0 {
            data[3..5].copy_from_slice(&dpi.to_be_bytes());
        }
        data[5] = self.lod;
        Ok(data)
    }
    fn changed(self, dpi: u16) -> Self {
        Self {
            x: dpi,
            y: if self.y == 0 { 0 } else { dpi },
            ..self
        }
    }
}

#[derive(Default)]
pub(crate) struct Pages {
    next: u8,
    stream: Vec<u8>,
    pub values: Option<Vec<u16>>,
}
impl Pages {
    pub fn next(&self) -> Option<u8> {
        (self.values.is_none() && self.next < 16).then_some(self.next)
    }
    pub fn accept(&mut self, direction: u8, page: u8, data: &[u8]) -> Result<()> {
        if data.len() != 16 || data[..3] != [0, direction, page] || self.next() != Some(page) {
            return Err(invalid());
        }
        self.stream.extend_from_slice(&data[3..]);
        self.next += 1;
        // A word may cross the 13-byte page boundary. Never pad individual pages.
        let words: Vec<_> = self
            .stream
            .as_chunks::<2>()
            .0
            .iter()
            .map(|v| u16::from_be_bytes([v[0], v[1]]))
            .collect();
        let Some(end) = words.iter().position(|w| *w == 0) else {
            return if self.next < 16 {
                Ok(())
            } else {
                Err(invalid())
            };
        };
        if self.stream[end * 2..].iter().any(|b| *b != 0) {
            return Err(invalid());
        }
        let mut values = Vec::new();
        let mut i = 0;
        while i < end {
            let start = words[i];
            if start >= 0xe000 || values.last().is_some_and(|last| *last > start) {
                return Err(invalid());
            }
            if i + 1 < end && words[i + 1] >= 0xe000 {
                let step = words[i + 1] & 0x1fff;
                let stop = *words
                    .get(i + 2)
                    .filter(|_| i + 2 < end)
                    .ok_or_else(invalid)?;
                if step == 0
                    || stop < start
                    || stop >= 0xe000
                    || !(stop - start).is_multiple_of(step)
                {
                    return Err(invalid());
                }
                values.extend((start..=stop).step_by(usize::from(step)));
                i += 3;
            } else {
                values.push(start);
                i += 1;
            }
        }
        values.dedup();
        if values.is_empty() {
            return Err(invalid());
        }
        self.values = Some(values);
        Ok(())
    }
}

/// Native firewall state, independently checks capabilities and the exact preserved fields.
#[derive(Default)]
pub(crate) struct Guard {
    counted: bool,
    flags: Option<u8>,
    pages: [Pages; 2],
    current: Option<Parameters>,
    target: Option<Parameters>,
    awaiting_readback: bool,
}
impl Guard {
    pub fn authorize(&mut self, function: u8, data: &[u8], long: bool) -> Result<()> {
        let allowed = if self.target.is_some() {
            self.awaiting_readback && !long && function == 5 && data == [0, 0, 0]
        } else if !long {
            match function {
                0 => data == [0, 0, 0],
                1 => self.counted && data == [0, 0, 0],
                2 => {
                    data.len() == 3
                        && data[0] == 0
                        && data[1] < 2
                        && self.flags.is_some_and(|f| data[1] == 0 || f & 1 != 0)
                        && self.pages[usize::from(data[1])].next() == Some(data[2])
                }
                5 => self.flags.is_some() && data == [0, 0, 0],
                _ => false,
            }
        } else if function == 6 && data.len() == 16 {
            if let Some(current) = self.current {
                let dpi = u16::from_be_bytes([data[1], data[2]]);
                if (100..=25600).contains(&current.x)
                    && self.pages[0]
                        .values
                        .as_ref()
                        .is_some_and(|v| v.contains(&dpi) && v.contains(&current.x))
                    && (current.y == 0
                        || self.pages[1]
                            .values
                            .as_ref()
                            .is_some_and(|v| v.contains(&dpi) && v.contains(&current.y)))
                    && current.setter(dpi).is_ok_and(|expected| data == expected)
                {
                    self.target = Some(current.changed(dpi));
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if allowed {
            Ok(())
        } else {
            Err(Error::Safety(
                "extended DPI command is outside the verified transaction".into(),
            ))
        }
    }
    pub fn observe(&mut self, function: u8, args: &[u8], data: &[u8]) -> Result<()> {
        match function {
            0 if data.first().is_some_and(|n| (1..=8).contains(n)) => self.counted = true,
            1 if data.len() >= 3 && data[0] == 0 && data[2] & !15 == 0 => {
                self.flags = Some(data[2]);
                self.pages = [Pages::default(), Pages::default()];
                self.current = None;
            }
            2 => self.pages[usize::from(args[1])].accept(args[1], args[2], data)?,
            5 => {
                let state = Parameters::parse(data, self.flags.ok_or_else(invalid)?)?;
                if let Some(target) = self.target {
                    if !self.awaiting_readback || state != target {
                        return Err(invalid());
                    }
                    self.target = None;
                    self.awaiting_readback = false;
                    self.current = None;
                    self.pages = [Pages::default(), Pages::default()];
                } else {
                    self.current = Some(state);
                }
            }
            6 if data.first() == Some(&0) && self.target.is_some() => self.awaiting_readback = true,
            _ => return Err(invalid()),
        }
        Ok(())
    }
}
