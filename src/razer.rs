//! Experimental Razer read-only protocol. No setting setters or serial queries.
//! Layout/routing facts: pinned OpenRazer source, see specs/razer-read-only.md.
use crate::error::{Error, Result};
use std::sync::OnceLock;

const PAYLOAD: usize = 90;
type Packet = [u8; PAYLOAD];

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadProfile {
    pub pid: u16,
    pub name: String,
    symbol: String,
    firmware_transaction: u8,
    dpi_transaction: u8,
    pub usb_interface: i32,
    minimum_wait_us: u64,
    source_declared_max_dpi: u16,
    source_record: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Profiles {
    schema_version: u8,
    purpose: String,
    upstream_commit: String,
    source: String,
    profiles: Vec<ReadProfile>,
    excluded_symbols: Vec<String>,
}
fn profiles() -> &'static Profiles {
    static DATA: OnceLock<Profiles> = OnceLock::new();
    DATA.get_or_init(|| {
        let p: Profiles =
            serde_json::from_str(include_str!("../research/razer-read-profiles.json"))
                .expect("invalid compiled Razer read profiles");
        assert_eq!(p.schema_version, 1);
        assert_eq!(p.purpose, "experimental_read_only_no_setters");
        assert_eq!(
            p.upstream_commit,
            "477f4d6d3ab7e9deb6310d5bd3742560c5cbe80d"
        );
        assert!(p
            .source
            .starts_with("https://github.com/openrazer/openrazer/"));
        assert!(p.profiles.iter().all(
            |m| m.symbol.starts_with("USB_DEVICE_ID_RAZER_") && !m.symbol.contains("BLUETOOTH")
        ));
        let _ = &p.excluded_symbols;
        p
    })
}
pub fn profile(pid: u16) -> Option<&'static ReadProfile> {
    profiles().profiles.iter().find(|p| p.pid == pid)
}
pub fn profile_count() -> usize {
    profiles().profiles.len()
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct Observation {
    pub model: String,
    pub vid: u16,
    pub pid: u16,
    pub firmware: String,
    pub dpi_x: u16,
    pub dpi_y: u16,
    pub source_declared_max_dpi: u16,
    pub source_record: String,
    pub protocol_source: String,
    pub evidence: String,
    pub transport: String,
    pub hardware_verified: bool,
    pub setters_available: bool,
    pub settings_modified: bool,
}

#[derive(Clone, Copy)]
enum Query {
    Firmware,
    Dpi,
}
fn checksum(packet: &Packet) -> u8 {
    packet[2..88].iter().copied().fold(0, |a, b| a ^ b)
}
fn request(profile: &ReadProfile, kind: Query) -> Packet {
    let mut p = [0; PAYLOAD];
    match kind {
        Query::Firmware => {
            p[1] = profile.firmware_transaction;
            p[5..8].copy_from_slice(&[2, 0, 0x81]);
        }
        Query::Dpi => {
            p[1] = profile.dpi_transaction;
            p[5..8].copy_from_slice(&[7, 4, 0x85]);
        }
    }
    // DPI selector is NOSTORE=0; no device mode, onboard or write commands exist here.
    p[88] = checksum(&p);
    p
}
fn response<'a>(request: &Packet, reply: &'a [u8]) -> Result<&'a [u8]> {
    let packet: &Packet = reply
        .try_into()
        .map_err(|_| Error::Protocol("Razer response length must be 90".into()))?;
    if packet[1] != request[1]
        || packet[2..8] != request[2..8]
        || packet[89] != 0
        || packet[88] != checksum(packet)
    {
        return Err(Error::Protocol(
            "Razer response correlation, header or checksum".into(),
        ));
    }
    if packet[0] != 2 {
        // Busy is not success, and never causes an automatic retransmission.
        return Err(Error::Rejected(packet[0]));
    }
    Ok(&packet[8..8 + usize::from(packet[5])])
}

struct ReadGuard {
    expected: [Packet; 2],
    next: usize,
    fault: bool,
}
impl ReadGuard {
    fn new(profile: &ReadProfile) -> Self {
        Self {
            expected: [
                request(profile, Query::Firmware),
                request(profile, Query::Dpi),
            ],
            next: 0,
            fault: false,
        }
    }
    fn authorize(&mut self, packet: &Packet) -> Result<()> {
        if self.fault || self.expected.get(self.next) != Some(packet) {
            self.fault = true;
            return Err(Error::Safety(
                "Razer transport permits only the two planned read requests".into(),
            ));
        }
        self.next += 1;
        Ok(())
    }
}
trait QueryIo {
    fn exchange(&mut self, request: &Packet) -> Result<Vec<u8>>;
}
fn observe(profile: &ReadProfile, io: &mut impl QueryIo) -> Result<Observation> {
    let firmware_request = request(profile, Query::Firmware);
    let firmware_reply = io.exchange(&firmware_request)?;
    let fw = response(&firmware_request, &firmware_reply)?;
    let dpi_request = request(profile, Query::Dpi);
    let dpi_reply = io.exchange(&dpi_request)?;
    let dpi = response(&dpi_request, &dpi_reply)?;
    let x = u16::from_be_bytes([dpi[1], dpi[2]]);
    let y = u16::from_be_bytes([dpi[3], dpi[4]]);
    if dpi[0] != 0
        || x == 0
        || y == 0
        || x > profile.source_declared_max_dpi
        || y > profile.source_declared_max_dpi
    {
        return Err(Error::Protocol(
            "Razer DPI storage selector or source bounds mismatch; no inferred limits".into(),
        ));
    }
    Ok(Observation {
        model: profile.name.clone(),
        vid: 0x1532,
        pid: profile.pid,
        firmware: format!("{}.{}", fw[0], fw[1]),
        dpi_x: x,
        dpi_y: y,
        source_declared_max_dpi: profile.source_declared_max_dpi,
        source_record: profile.source_record.clone(),
        protocol_source: profiles().source.clone(),
        evidence: "correlated_protocol_readback_not_physical_function_verification".into(),
        transport: "usb_hid_feature".into(),
        hardware_verified: false,
        setters_available: false,
        settings_modified: false,
    })
}

struct NativeReadOnly {
    device: hidapi::HidDevice,
    guard: ReadGuard,
    wait: std::time::Duration,
}
impl QueryIo for NativeReadOnly {
    fn exchange(&mut self, request: &Packet) -> Result<Vec<u8>> {
        self.guard.authorize(request)?;
        let result = (|| {
            let mut buffer = [0; PAYLOAD + 1]; // hidapi report ID 0 is not part of the 90-byte payload.
            buffer[1..].copy_from_slice(request);
            self.device
                .send_feature_report(&buffer)
                .map_err(|_| Error::Disconnected)?;
            std::thread::sleep(self.wait);
            let mut reply = [0; PAYLOAD + 1];
            let len = self
                .device
                .get_feature_report(&mut reply)
                .map_err(|_| Error::Disconnected)?;
            if len != reply.len() || reply[0] != 0 {
                return Err(Error::Protocol("Razer hidapi report size or ID".into()));
            }
            response(request, &reply[1..])?;
            Ok(reply[1..].to_vec())
        })();
        if result.is_err() {
            self.guard.fault = true;
        }
        result
    }
}

pub fn probe_native(index: usize) -> Result<Observation> {
    let api = crate::device::enumerate()?;
    let info = api.device_list().nth(index).ok_or(Error::NotFound)?;
    probe_info(&api, info)
}
pub(crate) fn probe_info(api: &hidapi::HidApi, info: &hidapi::DeviceInfo) -> Result<Observation> {
    if info.vendor_id() != 0x1532 || !matches!(info.bus_type(), hidapi::BusType::Usb) {
        return Err(Error::Safety(
            "Razer read probe requires a known USB Razer collection".into(),
        ));
    }
    let profile = profile(info.product_id()).ok_or(Error::Unsupported(
        "no reviewed Razer read profile for this PID".into(),
    ))?;
    if info.interface_number() != profile.usb_interface {
        return Err(Error::Safety(
            "Razer read query USB interface does not match its profile".into(),
        ));
    }
    let device = info.open_device(api).map_err(|_| Error::Access)?;
    let descriptor = crate::device::read_descriptor(&device)?;
    if !descriptor.reports.iter().any(|r| {
        r.kind == crate::descriptor::Kind::Feature && r.id == 0 && r.bits == (PAYLOAD * 8) as u32
    }) {
        return Err(Error::Safety(
            "Razer unnumbered feature90 descriptor required before queries".into(),
        ));
    }
    let mut io = NativeReadOnly {
        device,
        guard: ReadGuard::new(profile),
        wait: std::time::Duration::from_micros(profile.minimum_wait_us.max(1000)),
    };
    observe(profile, &mut io)
}

pub fn demo_observation() -> Result<Observation> {
    struct Demo {
        guard: ReadGuard,
    }
    impl QueryIo for Demo {
        fn exchange(&mut self, request: &Packet) -> Result<Vec<u8>> {
            self.guard.authorize(request)?;
            let mut reply = *request;
            reply[0] = 2;
            match request[7] {
                0x81 => reply[8..10].copy_from_slice(&[2, 8]),
                0x85 => reply[9..13].copy_from_slice(&[3, 32, 3, 32]),
                _ => return Err(Error::Safety("demo unknown command".into())),
            }
            reply[88] = checksum(&reply);
            Ok(reply.to_vec())
        }
    }
    let profile = profile(0x0084).ok_or(Error::NotFound)?;
    let mut result = observe(
        profile,
        &mut Demo {
            guard: ReadGuard::new(profile),
        },
    )?;
    result.transport = "simulation_no_device_access".into();
    result.evidence = "synthetic_fixtures_not_usb_capture".into();
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    struct Mock {
        expected: VecDeque<Packet>,
        replies: VecDeque<Result<Vec<u8>>>,
        sent: Vec<Packet>,
    }
    impl QueryIo for Mock {
        fn exchange(&mut self, request: &Packet) -> Result<Vec<u8>> {
            assert_eq!(self.expected.pop_front().as_ref(), Some(request));
            self.sent.push(*request);
            self.replies.pop_front().unwrap_or(Err(Error::Timeout))
        }
    }
    fn fixtures() -> (Packet, Packet, Vec<u8>, Vec<u8>) {
        // Independent wire-header fixtures, NOT captured hardware traffic.
        let mut fw = [0; PAYLOAD];
        fw[..8].copy_from_slice(&[0, 0x3f, 0, 0, 0, 2, 0, 0x81]);
        fw[88] = 0x83;
        let mut dpi = [0; PAYLOAD];
        dpi[..8].copy_from_slice(&[0, 0x3f, 0, 0, 0, 7, 4, 0x85]);
        dpi[88] = 0x86;
        let mut fw_reply = fw;
        fw_reply[0] = 2;
        fw_reply[8..10].copy_from_slice(&[1, 2]);
        fw_reply[88] = 0x80;
        let mut dpi_reply = dpi;
        dpi_reply[0] = 2;
        dpi_reply[9..13].copy_from_slice(&[3, 32, 6, 64]);
        dpi_reply[88] = 0xe3;
        (fw, dpi, fw_reply.to_vec(), dpi_reply.to_vec())
    }
    #[test]
    fn model_routing_and_transaction_variants_are_not_guessed() {
        assert_eq!(profile_count(), 102);
        for p in &profiles().profiles {
            assert!(matches!(p.firmware_transaction, 0x1f | 0x3f | 0xff));
            assert!(matches!(p.dpi_transaction, 0x1f | 0x3f | 0xff));
            assert!(matches!(p.usb_interface, 0 | 3));
            assert!((600..=400000).contains(&p.minimum_wait_us));
            assert!(!p.symbol.contains("BLUETOOTH"));
            assert!(p.source_declared_max_dpi >= 100);
        }
        assert_eq!(profile(0x0084).unwrap().dpi_transaction, 0x3f);
        assert_eq!(profile(0x008a).unwrap().dpi_transaction, 0xff);
        assert_eq!(profile(0x0099).unwrap().usb_interface, 3);
        assert!(profile(0xffff).is_none());
    }
    #[test]
    fn independent_fixtures_decode_firmware_and_separate_axes() {
        let p = profile(0x0084).unwrap();
        let (fw, dpi, a, b) = fixtures();
        assert_eq!(request(p, Query::Firmware), fw);
        assert_eq!(request(p, Query::Dpi), dpi);
        let mut mock = Mock {
            expected: [fw, dpi].into(),
            replies: [Ok(a), Ok(b)].into(),
            sent: vec![],
        };
        let result = observe(p, &mut mock).unwrap();
        assert_eq!(result.firmware, "1.2");
        assert_eq!((result.dpi_x, result.dpi_y), (800, 1600));
        assert!(
            !result.hardware_verified && !result.setters_available && !result.settings_modified
        );
        assert_eq!(mock.sent.len(), 2);
    }
    #[test]
    fn every_outgoing_mutation_and_reordered_request_is_blocked() {
        let p = profile(0x0084).unwrap();
        let (fw, dpi, _, _) = fixtures();
        for index in 0..PAYLOAD {
            let mut changed = fw;
            changed[index] ^= 1;
            let mut guard = ReadGuard::new(p);
            assert!(guard.authorize(&changed).is_err());
            assert!(guard.authorize(&fw).is_err());
        }
        assert!(ReadGuard::new(p).authorize(&dpi).is_err());
        let mut guard = ReadGuard::new(p);
        guard.authorize(&fw).unwrap();
        guard.authorize(&dpi).unwrap();
        assert!(guard.authorize(&fw).is_err());
    }
    #[test]
    fn malformed_busy_rejected_or_stale_firmware_never_reaches_second_query() {
        let p = profile(0x0084).unwrap();
        let (fw, _, good, _) = fixtures();
        let mut failures = vec![
            Err(Error::Timeout),
            Err(Error::Disconnected),
            Ok(vec![]),
            Ok(good[..89].to_vec()),
        ];
        for field in [1, 2, 4, 5, 6, 7, 88, 89] {
            let mut changed = good.clone();
            changed[field] ^= 1;
            failures.push(Ok(changed));
        }
        for status in [0, 1, 3, 4, 5] {
            let mut changed = good.clone();
            changed[0] = status;
            failures.push(Ok(changed));
        }
        for failure in failures {
            let mut mock = Mock {
                expected: [fw].into(),
                replies: [failure].into(),
                sent: vec![],
            };
            assert!(observe(p, &mut mock).is_err());
            assert_eq!(mock.sent.len(), 1);
        }
    }
    #[test]
    fn invalid_dpi_and_corrupt_checksum_are_not_observations() {
        let p = profile(0x0084).unwrap();
        let (fw, dpi, a, b) = fixtures();
        for (offset, value) in [(8, 1), (9, 0), (11, 0), (9, 255)] {
            let mut changed: Packet = b.clone().try_into().unwrap();
            changed[offset] = value;
            if value == 0 {
                changed[offset + 1] = 0;
            }
            changed[88] = checksum(&changed);
            let mut mock = Mock {
                expected: [fw, dpi].into(),
                replies: [Ok(a.clone()), Ok(changed.to_vec())].into(),
                sent: vec![],
            };
            assert!(observe(p, &mut mock).is_err());
            assert_eq!(mock.sent.len(), 2);
        }
        let mut corrupted = b;
        corrupted[12] ^= 1;
        assert!(response(&dpi, &corrupted).is_err());
    }
}
