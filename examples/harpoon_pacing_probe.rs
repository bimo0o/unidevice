use std::time::{Duration, Instant};
use unidevice::{
    device,
    error::{Error, Result},
};
fn run() -> Result<()> {
    let gap=match std::env::args().nth(1).as_deref(){None|Some("2")=>2,Some("1")=>1,_=>return Err(Error::Invalid("probe gap must be 1 or 2 ms".into()))};
    let api = device::enumerate()?;
    let mut matches = api.device_list().filter(|d| {
        (d.vendor_id(), d.product_id(), d.usage_page(), d.usage()) == (0x1b1c, 0x1b75, 0xffc2, 4)
            && matches!(d.bus_type(), hidapi::BusType::Usb)
    });
    let info = matches.next().ok_or(Error::NotFound)?;
    if matches.next().is_some() {
        return Err(Error::Ambiguous);
    }
    let h = info.open_device(&api).map_err(|_| Error::Access)?;
    if !device::read_descriptor(&h)?
        .reports
        .iter()
        .any(|r| r.kind == unidevice::descriptor::Kind::Feature && r.id == 0 && r.bytes() == 64)
    {
        return Err(Error::Safety("feature64 required".into()));
    }
    let start = Instant::now();
    let mut stage = 1;
    for i in 0..301 {
        let header = if i == 0 {
            [14, 1, 0, 0]
        } else {
            match (i - 1) % 3 {
                0 => [14, 0x13, 2, 0],
                1 => [14, 0x13, 5, 0],
                _ => [14, 0x13, 0xd0 | stage, 0],
            }
        };
        let mut tx = [0; 65];
        tx[1..5].copy_from_slice(&header);
        h.send_feature_report(&tx)
            .map_err(|_| Error::Disconnected)?;
        std::thread::sleep(Duration::from_millis(gap));
        let mut rx = [0; 65];
        let n = h
            .get_feature_report(&mut rx)
            .map_err(|_| Error::Disconnected)?;
        if n != 65 || rx[0] != 0 || rx[1..5] != header {
            return Err(Error::Protocol(format!(
                "pacing reply mismatch at request {i}"
            )));
        }
        if i == 0 && (rx[13..17] != [0x1c, 0x1b, 0x75, 0x1b] || rx[9..11] != [0x13, 3]) {
            return Err(Error::Unsupported("Harpoon 3.13 only".into()));
        }
        if header[2] == 2 {
            stage = rx[5];
            if !(1..=5).contains(&stage) {
                return Err(Error::Safety("invalid stage".into()));
            }
        }
    }
    println!(
        "301 read-only requests matched, elapsed_ms={}, requested_gap_ms={gap}",
        start.elapsed().as_millis()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
