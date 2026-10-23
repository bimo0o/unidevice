use crate::descriptor::Kind;
use crate::error::{Error, Result};

pub trait FeatureIo {
    fn send(&mut self, report: &[u8; 65]) -> Result<()>;
    fn receive(&mut self) -> Result<[u8; 65]>;
}

pub struct NativeFeature(hidapi::HidDevice);
impl NativeFeature {
    pub(crate) fn new(device: hidapi::HidDevice) -> Result<Self> {
        let descriptor = crate::device::read_descriptor(&device)?;
        if !descriptor
            .reports
            .iter()
            .any(|r| r.kind == Kind::Feature && r.id == 0 && r.bytes() == 64)
        {
            return Err(Error::Safety(
                "Corsair feature64 descriptor required".into(),
            ));
        }
        Ok(Self(device))
    }
}
impl FeatureIo for NativeFeature {
    fn send(&mut self, report: &[u8; 65]) -> Result<()> {
        if !allowed_report(report) {
            return Err(Error::Safety("Corsair physical command firewall".into()));
        }
        self.0
            .send_feature_report(report)
            .map_err(|_| Error::Disconnected)?;
        std::thread::sleep(std::time::Duration::from_millis(20));
        Ok(())
    }
    fn receive(&mut self) -> Result<[u8; 65]> {
        let mut report = [0; 65];
        if self
            .0
            .get_feature_report(&mut report)
            .map_err(|_| Error::Disconnected)?
            != 65
            || report[0] != 0
        {
            return Err(Error::Protocol("Corsair feature report size/ID".into()));
        }
        Ok(report)
    }
}

fn allowed_report(p: &[u8; 65]) -> bool {
    if p[0] != 0 || p[4] != 0 {
        return false;
    }
    if p[1] == 14 {
        return ((p[2] == 1 && p[3] == 0)
            || (p[2] == 0x13 && (matches!(p[3], 2 | 5) || (0xd0..=0xd5).contains(&p[3]))))
            && p[5..].iter().all(|v| *v == 0);
    }
    if p[1] != 7
        || p[2] != 0x13
        || !(0xd1..=0xd5).contains(&p[3])
        || p[5] > 1
        || p[13..].iter().any(|v| *v != 0)
    {
        return false;
    }
    [
        u16::from_le_bytes([p[6], p[7]]),
        u16::from_le_bytes([p[8], p[9]]),
    ]
    .iter()
    .all(|v| (100..=12000).contains(v))
}

pub fn open_harpoon() -> Result<Session<NativeFeature>> {
    let api = crate::device::enumerate()?;
    let mut candidates = api.device_list().filter(|d| {
        d.vendor_id() == 0x1b1c
            && d.product_id() == 0x1b75
            && d.usage_page() == 0xffc2
            && d.usage() == 4
            && matches!(d.bus_type(), hidapi::BusType::Usb)
    });
    let info = candidates.next().ok_or(Error::NotFound)?;
    if candidates.next().is_some() {
        return Err(Error::Ambiguous);
    }
    Session::connect(NativeFeature::new(
        info.open_device(&api).map_err(|_| Error::Access)?,
    )?)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    pub stage: u8,
    pub x: u16,
    pub y: u16,
    pub rgb: [u8; 3],
    xy: u8,
}

pub struct Session<T> {
    io: T,
    fault: bool,
}

impl<T: FeatureIo> Session<T> {
    pub fn connect(io: T) -> Result<Self> {
        let mut session = Self { io, fault: false };
        let identity = session.query(1, 0)?;
        if identity[12..16] != [0x1c, 0x1b, 0x75, 0x1b] || identity[8..10] != [0x13, 3] {
            return Err(Error::Unsupported(
                "Harpoon live control verified only for 1B75 firmware 3.13".into(),
            ));
        }
        session.state()?;
        Ok(session)
    }

    fn query(&mut self, field: u8, sub: u8) -> Result<[u8; 64]> {
        if self.fault {
            return Err(Error::Safety("Corsair session suspended".into()));
        }
        if !((field == 1 && sub == 0)
            || (field == 0x13 && (matches!(sub, 2 | 5) || (0xd0..=0xd5).contains(&sub))))
        {
            return Err(Error::Safety("Corsair query outside allowlist".into()));
        }
        let mut request = [0; 65];
        request[1..5].copy_from_slice(&[0x0e, field, sub, 0]);
        let result = (|| {
            self.io.send(&request)?;
            let response = self.io.receive()?;
            if response[0] != 0 || response[1..5] != request[1..5] {
                return Err(Error::Protocol(
                    "Corsair stale or mismatched response".into(),
                ));
            }
            let mut payload = [0; 64];
            payload.copy_from_slice(&response[1..]);
            Ok(payload)
        })();
        if result.is_err() {
            self.fault = true;
        }
        result
    }

    pub fn state(&mut self) -> Result<State> {
        let result = (|| {
            let stage = self.query(0x13, 2)?[4];
            let mask = self.query(0x13, 5)?[4];
            if !(1..=5).contains(&stage) || mask & !0x3f != 0 || mask & (1 << stage) == 0 {
                return Err(Error::Safety("invalid or inactive Corsair stage".into()));
            }
            let p = self.query(0x13, 0xd0 | stage)?;
            let state = State {
                stage,
                x: u16::from_le_bytes([p[5], p[6]]),
                y: u16::from_le_bytes([p[7], p[8]]),
                rgb: [p[9], p[10], p[11]],
                xy: p[4],
            };
            if state.xy > 1
                || !(100..=12000).contains(&state.x)
                || !(100..=12000).contains(&state.y)
            {
                return Err(Error::Safety("unsupported Corsair stage data".into()));
            }
            Ok(state)
        })();
        if result.is_err() {
            self.fault = true;
        }
        result
    }

    pub fn set_dpi(&mut self, dpi: u16) -> Result<State> {
        if !(100..=12000).contains(&dpi) || dpi % 100 != 0 {
            return Err(Error::Invalid(
                "Harpoon application policy: 100..12000, step 100".into(),
            ));
        }
        let before = self.state()?;
        let mut after = before.clone();
        after.x = dpi;
        after.y = dpi;
        after.xy = 0;
        self.update(before, after)
    }

    pub fn set_stage_rgb(&mut self, rgb: [u8; 3]) -> Result<State> {
        self.set_rgb_expected(rgb, None)
    }
    pub(crate) fn set_rgb_expected(&mut self, rgb: [u8; 3], stage: Option<u8>) -> Result<State> {
        let before = self.state()?;
        if stage.is_some_and(|stage| stage != before.stage) {
            self.fault = true;
            return Err(Error::Safety(
                "DPI stage changed during RGB animation".into(),
            ));
        }
        let mut after = before.clone();
        after.rgb = rgb;
        self.update(before, after)
    }

    fn update(&mut self, before: State, after: State) -> Result<State> {
        if self.fault {
            return Err(Error::Safety("Corsair session suspended".into()));
        }
        if before == after {
            return Ok(before);
        }
        let result = (|| {
            // Re-read immediately before the single setter; never change the stage or mask.
            if self.state()? != before {
                return Err(Error::Safety("Corsair state changed before apply".into()));
            }
            let mut report = [0; 65];
            report[1..6].copy_from_slice(&[7, 0x13, 0xd0 | after.stage, 0, after.xy]);
            report[6..8].copy_from_slice(&after.x.to_le_bytes());
            report[8..10].copy_from_slice(&after.y.to_le_bytes());
            report[10..13].copy_from_slice(&after.rgb);
            self.io.send(&report)?;
            let actual = self.state()?;
            if actual != after {
                return Err(Error::Protocol(
                    "Corsair write readback mismatch; no retry".into(),
                ));
            }
            Ok(actual)
        })();
        if result.is_err() {
            self.fault = true;
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };
    struct Fake {
        replies: VecDeque<Result<[u8; 65]>>,
        sends: Arc<Mutex<Vec<[u8; 65]>>>,
    }
    impl FeatureIo for Fake {
        fn send(&mut self, p: &[u8; 65]) -> Result<()> {
            self.sends.lock().unwrap().push(*p);
            Ok(())
        }
        fn receive(&mut self) -> Result<[u8; 65]> {
            self.replies.pop_front().unwrap_or(Err(Error::Timeout))
        }
    }
    fn reply(field: u8, sub: u8, data: &[u8]) -> Result<[u8; 65]> {
        let mut r = [0; 65];
        r[1..5].copy_from_slice(&[14, field, sub, 0]);
        r[5..5 + data.len()].copy_from_slice(data);
        Ok(r)
    }
    fn state(dpi: u16, color: [u8; 3]) -> Vec<Result<[u8; 65]>> {
        let [lo, hi] = dpi.to_le_bytes();
        vec![
            reply(0x13, 2, &[1]),
            reply(0x13, 5, &[3]),
            reply(
                0x13,
                0xd1,
                &[0, lo, hi, lo, hi, color[0], color[1], color[2]],
            ),
        ]
    }
    type Sent = Arc<Mutex<Vec<[u8; 65]>>>;
    fn setup(extra: Vec<Result<[u8; 65]>>) -> (Session<Fake>, Sent) {
        let mut id = reply(1, 0, &[]).unwrap();
        id[9..11].copy_from_slice(&[0x13, 3]);
        id[13..17].copy_from_slice(&[0x1c, 0x1b, 0x75, 0x1b]);
        let sends = Arc::new(Mutex::new(Vec::new()));
        let mut replies = vec![Ok(id)];
        replies.extend(state(1000, [255, 0, 0]));
        replies.extend(extra);
        (
            Session::connect(Fake {
                replies: replies.into(),
                sends: sends.clone(),
            })
            .unwrap(),
            sends,
        )
    }
    #[test]
    fn active_stage_write_preserves_rgb_and_reads_back() {
        let mut extra = state(1000, [255, 0, 0]);
        extra.extend(state(1000, [255, 0, 0]));
        extra.extend(state(800, [255, 0, 0]));
        let (mut s, sends) = setup(extra);
        assert_eq!(s.set_dpi(800).unwrap().x, 800);
        let sends = sends.lock().unwrap();
        let sets: Vec<_> = sends.iter().filter(|p| p[1] == 7).collect();
        assert_eq!(sets.len(), 1);
        assert_eq!(
            &sets[0][1..13],
            &[7, 0x13, 0xd1, 0, 0, 0x20, 3, 0x20, 3, 255, 0, 0]
        );
        assert!(sets[0][13..].iter().all(|b| *b == 0));
    }
    #[test]
    fn failed_readback_latches_session_and_never_retries() {
        let mut extra = state(1000, [255, 0, 0]);
        extra.extend(state(1000, [255, 0, 0]));
        extra.push(Err(Error::Timeout));
        let (mut s, sends) = setup(extra);
        assert!(s.set_dpi(800).is_err());
        let count = sends.lock().unwrap().len();
        assert!(s.set_dpi(1600).is_err());
        assert_eq!(sends.lock().unwrap().len(), count);
    }
    #[test]
    fn invalid_values_and_unknown_queries_never_reach_device() {
        let (mut s, sends) = setup(vec![]);
        let count = sends.lock().unwrap().len();
        for dpi in [0, 99, 101, 12001, u16::MAX] {
            assert!(s.set_dpi(dpi).is_err());
        }
        for field in [2, 4, 5, 0x17, 0xff] {
            assert!(s.query(field, 0).is_err());
        }
        assert_eq!(sends.lock().unwrap().len(), count);
    }
    #[test]
    fn rgb_preserves_dpi_and_flag() {
        let mut extra = state(1000, [255, 0, 0]);
        extra.extend(state(1000, [255, 0, 0]));
        extra.extend(state(1000, [0, 0, 255]));
        let (mut s, sends) = setup(extra);
        assert_eq!(s.set_stage_rgb([0, 0, 255]).unwrap().x, 1000);
        let packets = sends.lock().unwrap();
        let p = packets.iter().find(|p| p[1] == 7).unwrap();
        assert_eq!(&p[6..13], &[0xe8, 3, 0xe8, 3, 0, 0, 255]);
    }
    #[test]
    fn changed_stage_or_corrupt_reply_prevents_setter() {
        let mut extra = state(1000, [255, 0, 0]);
        extra.push(reply(0x13, 2, &[2]));
        extra.push(reply(0x13, 5, &[3]));
        let (mut s, sends) = setup(extra);
        assert!(s.set_dpi(800).is_err());
        assert!(sends.lock().unwrap().iter().all(|p| p[1] != 7));
        let (mut s, sends) = setup(vec![reply(0x13, 5, &[3])]);
        assert!(s.set_dpi(800).is_err());
        let n = sends.lock().unwrap().len();
        assert!(s.state().is_err());
        assert_eq!(n, sends.lock().unwrap().len());
    }
    #[test]
    fn physical_firewall_blocks_reset_flash_wrong_report_and_padding() {
        let mut p = [0; 65];
        p[1..5].copy_from_slice(&[14, 1, 0, 0]);
        assert!(allowed_report(&p));
        for field in [2, 4, 5, 0x0a, 0x17, 0xff] {
            p[1..5].copy_from_slice(&[7, field, 0, 0]);
            assert!(!allowed_report(&p));
        }
        p[1..13].copy_from_slice(&[7, 0x13, 0xd1, 0, 0, 0x20, 3, 0x20, 3, 255, 0, 0]);
        assert!(allowed_report(&p));
        p[4] = 1;
        assert!(!allowed_report(&p));
        p[4] = 0;
        p[64] = 1;
        assert!(!allowed_report(&p));
    }
    #[test]
    fn effect_cannot_follow_a_different_physical_dpi_stage() {
        let mut extra = state(1000, [255, 0, 0]);
        extra[0] = reply(0x13, 2, &[2]);
        extra[1] = reply(0x13, 5, &[7]);
        extra[2] = reply(0x13, 0xd2, &[0, 0xe8, 3, 0xe8, 3, 255, 0, 0]);
        let (mut session, sends) = setup(extra);
        assert!(session.set_rgb_expected([0, 0, 255], Some(1)).is_err());
        assert!(sends.lock().unwrap().iter().all(|p| p[1] != 7));
        let count = sends.lock().unwrap().len();
        assert!(session.state().is_err());
        assert_eq!(count, sends.lock().unwrap().len());
    }
}
