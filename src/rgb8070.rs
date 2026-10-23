//! Bounded 0x8070 RGB settings, no HID access. Wire facts: pinned OpenLogi source.
use crate::error::{Error, Result};

fn bad() -> Error {
    Error::Protocol("invalid 8070 zone, effect or parameters".into())
}
fn word(data: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([data[i], data[i + 1]])
}
fn long(data: &[u8]) -> Result<()> {
    if data.len() == 16 {
        Ok(())
    } else {
        Err(bad())
    }
}

pub(crate) fn zone_count(data: &[u8]) -> Result<u8> {
    long(data)?;
    let ext = word(data, 3);
    if data[0] == 0 || data[0] > 8 || ext & !31 != 0 || ext & 1 == 0 || ext & 8 != 0 {
        return Err(Error::Unsupported(
            "8070 needs 1..8 RGB zones with RAM effect readback".into(),
        ));
    }
    Ok(data[0])
}

#[derive(Clone, Debug)]
pub(crate) struct Zone {
    pub index: u8,
    pub effects: Vec<u16>,
}
impl Zone {
    pub fn info(index: u8, data: &[u8]) -> Result<Self> {
        long(data)?;
        if data[0] != index
            || !(1..=11).contains(&word(data, 1))
            || !(1..=32).contains(&data[3])
            || data[4] & !7 != 0
        {
            return Err(bad());
        }
        Ok(Self {
            index,
            effects: vec![u16::MAX; usize::from(data[3])],
        })
    }
    pub fn effect_info(&mut self, index: u8, data: &[u8]) -> Result<()> {
        long(data)?;
        if data[..2] != [self.index, index] {
            return Err(bad());
        }
        let slot = self.effects.get_mut(usize::from(index)).ok_or_else(bad)?;
        if *slot != u16::MAX {
            return Err(bad());
        }
        *slot = word(data, 2);
        Ok(())
    }
    fn fixed_index(&self) -> Result<u8> {
        let indexes: Vec<_> = self
            .effects
            .iter()
            .enumerate()
            .filter(|(_, id)| **id == 1)
            .collect();
        if indexes.len() != 1 {
            return Err(Error::Unsupported(
                "8070 zone needs one advertised fixed-color effect".into(),
            ));
        }
        Ok(indexes[0].0 as u8)
    }
    fn validate(&self, state: &Setting) -> Result<()> {
        if state.zone != self.index
            || self.effects.contains(&u16::MAX)
            || self.effects.get(usize::from(state.effect_index)) != Some(&state.effect_id)
        {
            return Err(bad());
        }
        state.validate_parameters()
    }
    pub fn fixed(&self, rgb: [u8; 3]) -> Result<Setting> {
        let mut params = [0; 10];
        params[..3].copy_from_slice(&rgb);
        Ok(Setting {
            zone: self.index,
            effect_index: self.fixed_index()?,
            effect_id: 1,
            params,
        })
    }
    pub fn read(&self, data: &[u8]) -> Result<Setting> {
        long(data)?;
        if data[0] != self.index || data[12..].iter().any(|v| *v != 0) {
            return Err(bad());
        }
        let effect_id = *self.effects.get(usize::from(data[1])).ok_or_else(bad)?;
        let mut params = [0; 10];
        params.copy_from_slice(&data[2..12]);
        let state = Setting {
            zone: self.index,
            effect_index: data[1],
            effect_id,
            params,
        };
        self.validate(&state)?;
        Ok(state)
    }
}

/// Settings only, not command packets. No deserialization or arbitrary report import.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Setting {
    zone: u8,
    effect_index: u8,
    effect_id: u16,
    params: [u8; 10],
}
impl Setting {
    pub(crate) fn zone_index(&self) -> u8 {
        self.zone
    }
    fn validate_parameters(&self) -> Result<()> {
        let p = &self.params;
        let period = |offset| (100..=60000).contains(&word(p, offset));
        let valid = match self.effect_id {
            0 => p.iter().all(|v| *v == 0),
            1 => p[3..].iter().all(|v| *v == 0),
            3 => p[3..5] == [0, 0] && period(5) && p[7] <= 100 && p[8..] == [0, 0],
            10 => period(3) && p[5] <= 6 && p[6] <= 100 && p[7..] == [0, 0, 0],
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(Error::Unsupported(
                "8070 original effect cannot yet be safely preserved; RGB disabled".into(),
            ))
        }
    }
    fn payload(&self) -> [u8; 16] {
        let mut p = [0; 16];
        p[0] = self.zone;
        p[1] = self.effect_index;
        p[2..12].copy_from_slice(&self.params);
        // Persistence byte 12 ALWAYS 0 (RAM), reserved bytes 13..15 ALWAYS zero.
        p
    }
    pub fn color_hint(&self) -> [u8; 3] {
        [self.params[0], self.params[1], self.params[2]]
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct Snapshot {
    pub(crate) settings: Vec<Setting>,
}
impl Snapshot {
    pub fn zones(&self) -> usize {
        self.settings.len()
    }
    pub fn color_hint(&self) -> [u8; 3] {
        self.settings
            .first()
            .map(Setting::color_hint)
            .unwrap_or([0; 3])
    }
    pub fn portable_color(&self) -> Option<[u8; 3]> {
        let first = self.settings.first()?;
        (self
            .settings
            .iter()
            .all(|s| s.effect_id == 1 && s.color_hint() == first.color_hint()))
        .then_some(first.color_hint())
    }
}

#[derive(Default)]
pub(crate) struct Guard {
    count: Option<u8>,
    zones: Vec<Option<Zone>>,
    fresh: Vec<Option<Setting>>,
    target: Option<Setting>,
    acked: bool,
}
impl Guard {
    pub fn awaits_readback(&self) -> bool {
        self.target.is_some()
    }
    pub fn authorize(&mut self, function: u8, args: &[u8], is_long: bool) -> Result<()> {
        let allowed = if let Some(target) = &self.target {
            self.acked && !is_long && function == 14 && args == [target.zone, 0, 0]
        } else if is_long && function == 3 && args.len() == 16 {
            let zone = self
                .zones
                .get(usize::from(args[0]))
                .and_then(Option::as_ref)
                .ok_or_else(bad)?;
            let state = zone.read(args)?;
            if self.fresh[usize::from(args[0])].is_none() {
                false
            } else {
                self.target = Some(state);
                true
            }
        } else if !is_long && args.len() == 3 {
            match function {
                0 => args == [0, 0, 0],
                1 => self.count.is_some_and(|n| args[0] < n) && args[1..] == [0, 0],
                2 => {
                    args[2] == 0
                        && self
                            .zones
                            .get(usize::from(args[0]))
                            .and_then(Option::as_ref)
                            .is_some_and(|z| z.effects.get(usize::from(args[1])) == Some(&u16::MAX))
                }
                14 => {
                    args[1..] == [0, 0]
                        && self
                            .zones
                            .get(usize::from(args[0]))
                            .and_then(Option::as_ref)
                            .is_some_and(|z| !z.effects.contains(&u16::MAX))
                }
                _ => false,
            }
        } else {
            false
        };
        if allowed {
            Ok(())
        } else {
            Err(Error::Safety(
                "8070 command outside bounded RAM transaction".into(),
            ))
        }
    }
    pub fn observe(&mut self, function: u8, args: &[u8], data: &[u8]) -> Result<()> {
        match function {
            0 => {
                let count = match zone_count(data) {
                    Ok(n) => n,
                    Err(Error::Unsupported(_)) if self.count.is_none() => return Ok(()),
                    Err(e) => return Err(e),
                };
                if self.count.is_some_and(|old| old != count) {
                    return Err(bad());
                }
                if self.count.is_none() {
                    self.zones = vec![None; usize::from(count)];
                    self.fresh = vec![None; usize::from(count)];
                }
                self.count = Some(count);
            }
            1 => {
                self.zones[usize::from(args[0])] = Some(Zone::info(args[0], data)?);
                self.fresh[usize::from(args[0])] = None;
            }
            2 => self.zones[usize::from(args[0])]
                .as_mut()
                .ok_or_else(bad)?
                .effect_info(args[1], data)?,
            14 => {
                let state = match self.zones[usize::from(args[0])]
                    .as_ref()
                    .ok_or_else(bad)?
                    .read(data)
                {
                    Ok(s) => s,
                    Err(Error::Unsupported(_)) if self.target.is_none() => {
                        self.fresh[usize::from(args[0])] = None;
                        return Ok(());
                    }
                    Err(e) => return Err(e),
                };
                if let Some(target) = &self.target {
                    if !self.acked || &state != target {
                        return Err(bad());
                    }
                    self.target = None;
                    self.acked = false;
                    self.fresh[usize::from(args[0])] = None;
                } else {
                    self.fresh[usize::from(args[0])] = Some(state);
                }
            }
            3 if self.target.is_some() => {
                self.acked = true;
            }
            _ => return Err(bad()),
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Layout {
    pub index: u8,
    pub zones: Vec<Zone>,
}
impl Layout {
    pub fn validate(&self) -> Result<()> {
        for z in &self.zones {
            z.fixed_index()?;
        }
        Ok(())
    }
    pub fn payload(&self, setting: &Setting) -> Result<[u8; 16]> {
        self.zones
            .get(usize::from(setting.zone))
            .ok_or_else(bad)?
            .validate(setting)?;
        Ok(setting.payload())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hidpp::Session,
        safety::GuardedTransport,
        transport::{ScriptedTransport, Transport},
    };
    #[derive(Default)]
    struct Plan {
        requests: Vec<Vec<u8>>,
        replies: Vec<Result<Vec<u8>>>,
        sw: u8,
    }
    impl Plan {
        fn step(
            &mut self,
            feature: u8,
            function: u8,
            args: &[u8],
            payload: &[u8],
            long_reply: bool,
        ) {
            self.sw = self.sw % 15 + 1;
            let address = function * 16 + self.sw;
            let mut request = vec![
                if args.len() == 16 { 0x11 } else { 0x10 },
                0xff,
                feature,
                address,
            ];
            request.extend_from_slice(args);
            let mut reply = vec![if long_reply { 0x11 } else { 0x10 }, 0xff, feature, address];
            reply.extend_from_slice(payload);
            reply.resize(if long_reply { 20 } else { 7 }, 0);
            self.requests.push(request);
            self.replies.push(Ok(reply));
        }
        fn transport(self) -> GuardedTransport<ScriptedTransport> {
            GuardedTransport::new(ScriptedTransport {
                expected: self.requests.into(),
                replies: self.replies.into(),
                ..Default::default()
            })
        }
    }
    fn fixed(zone: u8, effect: u8, color: [u8; 3]) -> [u8; 16] {
        let mut p = [0; 16];
        p[..5].copy_from_slice(&[zone, effect, color[0], color[1], color[2]]);
        p
    }
    fn cycle() -> [u8; 16] {
        let mut p = [0; 16];
        p[..2].copy_from_slice(&[1, 1]);
        p[7..10].copy_from_slice(&[7, 0xd0, 75]);
        p
    }
    fn base() -> Plan {
        let mut p = Plan::default();
        p.step(0, 1, &[0, 0, 0x5a], &[2, 0, 0x5a], false);
        p.step(0, 0, &[0x22, 1, 0], &[7, 0, 1], false);
        p.step(7, 0, &[0, 0, 0], &[1, 0, 0], false);
        p
    }
    fn discovery(p: &mut Plan) {
        p.step(0, 0, &[0x80, 0x70, 0], &[12, 0, 7], false);
        p.step(12, 0, &[0, 0, 0], &[2, 0, 0, 0, 1], true);
        p.step(12, 1, &[0, 0, 0], &[0, 0, 2, 2, 7], true);
        p.step(12, 2, &[0, 0, 0], &[0, 0, 0, 0], true);
        p.step(12, 2, &[0, 1, 0], &[0, 1, 0, 1], true);
        p.step(12, 1, &[1, 0, 0], &[1, 0, 3, 2, 7], true);
        p.step(12, 2, &[1, 0, 0], &[1, 0, 0, 1], true);
        p.step(12, 2, &[1, 1, 0], &[1, 1, 0, 3], true);
    }
    fn read(p: &mut Plan, states: &[[u8; 16]; 2]) {
        for (i, state) in states.iter().enumerate() {
            p.step(12, 14, &[i as u8, 0, 0], state, true);
        }
    }
    fn update(p: &mut Plan, before: &[[u8; 16]; 2], after: &[[u8; 16]; 2]) {
        p.step(7, 2, &[0, 0, 0], &[0, 3, 32], false);
        read(p, before);
        for (old, new) in before.iter().zip(after) {
            if old == new {
                continue;
            }
            p.step(12, 3, new, &[0, 0, 0], false);
            p.step(12, 14, &[new[0], 0, 0], new, true);
        }
        p.step(7, 2, &[0, 0, 0], &[0, 3, 32], false);
    }
    fn full() -> Plan {
        // Synthetic plan, NOT physical USB capture: different fixed-effect indexes,
        // animated baseline, two zones, software-ID wrap, then an independent DPI apply.
        let mut p = base();
        discovery(&mut p);
        let original = [fixed(0, 1, [8, 16, 24]), cycle()];
        let target = [fixed(0, 1, [10, 20, 30]), fixed(1, 0, [10, 20, 30])];
        read(&mut p, &original);
        update(&mut p, &original, &target);
        update(&mut p, &target, &original);
        p.step(7, 1, &[0, 0, 0], &[0, 1, 0x90, 3, 32, 6, 0x40], true);
        p.step(7, 2, &[0, 0, 0], &[0, 3, 32], false);
        p.step(7, 3, &[0, 6, 0x40], &[0, 6, 0x40], false);
        p.step(7, 2, &[0, 0, 0], &[0, 6, 0x40], false);
        p
    }
    fn prefix(n: usize) -> GuardedTransport<ScriptedTransport> {
        let plan = full();
        let requests = plan.requests.clone();
        let mut t = plan.transport();
        for request in requests.iter().take(n) {
            t.write(request).unwrap();
            t.read(1).unwrap();
        }
        t
    }
    fn first_setter() -> usize {
        full()
            .requests
            .iter()
            .position(|r| r[2] == 12 && r[3] >> 4 == 3)
            .unwrap()
    }
    #[test]
    fn multizone_rgb_restores_original_pattern_and_dpi_firewall_remains_independent() {
        let plan = full();
        let expected = plan.requests.clone();
        let mut s = Session::connect(plan.transport()).unwrap();
        assert!(s.discover_rgb().unwrap());
        let original = s.rgb_snapshot().unwrap();
        assert_eq!(original.zones(), 2);
        assert_eq!(original.portable_color(), None);
        let changed = s.set_rgb([10, 20, 30]).unwrap();
        assert_eq!(changed.portable_color(), Some([10, 20, 30]));
        assert_eq!(s.restore_rgb(&original).unwrap(), original);
        assert_eq!(s.set_dpi(1600).unwrap(), 1600);
        let t = s.into_transport();
        assert_eq!(t.inner().writes, expected);
        assert!(t.inner().expected.is_empty());
        for r in t
            .inner()
            .writes
            .iter()
            .filter(|r| r[2] == 12 && r[3] >> 4 == 3)
        {
            assert_eq!(&r[16..], &[0, 0, 0, 0]);
        }
    }
    #[test]
    fn persistence_reserved_bytes_wrong_zones_and_forbidden_functions_never_reach_io() {
        let start = first_setter();
        for offset in [0, 1, 2, 3, 4, 5, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19] {
            let mut t = prefix(start);
            let mut r = full().requests[start].clone();
            r[offset] ^= if offset == 3 { 0x10 } else { 1 };
            assert!(t.write(&r).is_err(), "offset {offset}");
            assert_eq!(t.inner().writes.len(), start);
        }
        for function in [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 15] {
            let mut t = prefix(start);
            let mut r = full().requests[start].clone();
            r[3] = (function << 4) | 1;
            assert!(t.write(&r).is_err());
            assert_eq!(t.inner().writes.len(), start);
        }
        for prefix_len in [0, 3, 4, 5, 8, 11, 12] {
            let mut t = prefix(prefix_len);
            assert!(t.write(&full().requests[start]).is_err());
            assert_eq!(t.inner().writes.len(), prefix_len);
        }
    }
    #[test]
    fn bad_readback_stops_after_one_write_without_automatic_restore_or_retry() {
        let start = first_setter();
        for error in [
            Error::Timeout,
            Error::Disconnected,
            Error::Protocol("truncated".into()),
        ] {
            let mut p = full();
            p.replies[start + 1] = Err(error);
            let mut s = Session::connect(p.transport()).unwrap();
            s.discover_rgb().unwrap();
            s.rgb_snapshot().unwrap();
            assert!(s.set_rgb([10, 20, 30]).is_err());
            assert!(s.set_rgb([30, 20, 10]).is_err());
            let t = s.into_transport();
            assert_eq!(t.inner().writes.len(), start + 2);
            assert_eq!(
                t.inner()
                    .writes
                    .iter()
                    .filter(|r| r[2] == 12 && r[3] >> 4 == 3)
                    .count(),
                1
            );
        }
        for offset in [4, 5, 6, 12, 16] {
            let mut p = full();
            p.replies[start + 1].as_mut().unwrap()[offset] ^= 1;
            let mut s = Session::connect(p.transport()).unwrap();
            s.discover_rgb().unwrap();
            s.rgb_snapshot().unwrap();
            assert!(s.set_rgb([10, 20, 30]).is_err());
            assert_eq!(s.into_transport().inner().writes.len(), start + 2);
        }
    }
    #[test]
    fn absent_or_unreadable_rgb_does_not_enable_a_setter_or_break_dpi() {
        for info in [None, Some(vec![2, 0, 0, 0, 0]), Some(vec![2, 0, 0, 0, 9])] {
            let mut p = base();
            p.step(
                0,
                0,
                &[0x80, 0x70, 0],
                if info.is_some() {
                    &[12, 0, 7]
                } else {
                    &[0, 0, 0]
                },
                false,
            );
            if let Some(info) = info {
                p.step(12, 0, &[0, 0, 0], &info, true);
            }
            p.step(7, 2, &[0, 0, 0], &[0, 3, 32], false);
            let mut s = Session::connect(p.transport()).unwrap();
            assert!(!s.discover_rgb().unwrap());
            assert!(s.set_rgb([1, 2, 3]).is_err());
            assert_eq!(s.read_dpi().unwrap(), 800);
        }
        let mut plan = base();
        discovery(&mut plan);
        plan.step(12, 0, &[0, 0, 0], &[2, 0, 0, 0, 0], true);
        let requests = plan.requests.clone();
        let mut t = plan.transport();
        for r in requests.iter().take(requests.len() - 1) {
            t.write(r).unwrap();
            t.read(1).unwrap();
        }
        t.write(requests.last().unwrap()).unwrap();
        assert!(t.read(1).is_err()); // Previously granted RGB capabilities cannot silently disappear.
        assert!(t.write(&full().requests[first_setter()]).is_err());
    }
    #[test]
    fn malformed_state_and_unrecognized_original_effect_never_reach_a_setter() {
        for (reply, offset, value) in [(5, 4, 1), (7, 4, 1), (11, 5, 99), (11, 16, 1)] {
            let mut p = full();
            p.replies[reply].as_mut().unwrap()[offset] = value;
            let mut s = Session::connect(p.transport()).unwrap();
            if s.discover_rgb().is_ok() {
                assert!(s.rgb_snapshot().is_err());
            }
            assert_eq!(
                s.into_transport()
                    .inner()
                    .writes
                    .iter()
                    .filter(|r| r[2] == 12 && r[3] >> 4 == 3)
                    .count(),
                0
            );
        }
    }
    #[test]
    fn backup_contains_per_zone_effects_without_commands_or_unique_ids() {
        let mut s = Session::connect(full().transport()).unwrap();
        s.discover_rgb().unwrap();
        let snapshot = s.rgb_snapshot().unwrap();
        let text = serde_json::to_string(&snapshot).unwrap();
        assert!(text.contains("\"effect_id\":3"));
        for forbidden in ["serial", "path", "report_id", "command", "firmware"] {
            assert!(!text.contains(forbidden));
        }
        assert_eq!(snapshot.portable_color(), None);
    }
    #[test]
    fn zone_backup_is_synced_exclusive_and_retains_animated_baseline() {
        let mut s = Session::connect(full().transport()).unwrap();
        s.discover_rgb().unwrap();
        let snapshot = s.rgb_snapshot().unwrap();
        let file = crate::recovery::save_logitech_settings(800, &snapshot).unwrap();
        let path = crate::storage::root().unwrap().join(file);
        let text = crate::storage::read(&path).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["format"], "unidevice-logitech8070-settings");
        assert_eq!(value["lighting"]["settings"].as_array().unwrap().len(), 2);
        assert_eq!(value["lighting"]["settings"][1]["effect_id"], 3);
        assert!(value["portable_rgb"].is_null());
        assert!(crate::storage::save_new(&path, "overwrite").is_err());
        assert_eq!(crate::storage::read(&path).unwrap(), text);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn outstanding_rgb_write_blocks_other_features_and_duplicate_setters() {
        let start = first_setter();
        for packet in [
            vec![0x10, 0xff, 7, 0x21, 0, 0, 0],
            full().requests[start].clone(),
            vec![0x10, 0xff, 0, 1, 0x80, 0x70, 0],
        ] {
            let mut t = prefix(start + 1);
            assert!(t.write(&packet).is_err());
            assert_eq!(t.inner().writes.len(), start + 1);
        }
    }
    #[test]
    fn rgb_works_with_extended_dpi_without_changing_optional_fields() {
        let mut p = Plan::default();
        p.step(0, 1, &[0, 0, 0x5a], &[2, 0, 0x5a], false);
        p.step(0, 0, &[0x22, 1, 0], &[0, 0, 0], false);
        p.step(0, 0, &[0x22, 2, 0], &[7, 0, 0], false);
        p.step(7, 0, &[0, 0, 0], &[1, 0, 0], false);
        p.step(7, 1, &[0, 0, 0], &[0, 5, 0], false);
        discovery(&mut p);
        let original = [fixed(0, 1, [8, 16, 24]), cycle()];
        let target = [fixed(0, 1, [10, 20, 30]), fixed(1, 0, [10, 20, 30])];
        read(&mut p, &original);
        let dpi = [0, 3, 32, 1, 0x90, 0, 0, 0, 0, 0];
        p.step(7, 5, &[0, 0, 0], &dpi, true);
        read(&mut p, &original);
        for state in &target {
            p.step(12, 3, state, &[0, 0, 0], false);
            p.step(12, 14, &[state[0], 0, 0], state, true);
        }
        p.step(7, 5, &[0, 0, 0], &dpi, true);
        let expected = p.requests.clone();
        let mut s = Session::connect(p.transport()).unwrap();
        assert!(s.discover_rgb().unwrap());
        s.rgb_snapshot().unwrap();
        assert_eq!(
            s.set_rgb([10, 20, 30]).unwrap().portable_color(),
            Some([10, 20, 30])
        );
        assert_eq!(s.into_transport().inner().writes, expected);
    }
}
