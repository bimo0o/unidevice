use unidevice::{
    descriptor::Kind,
    device,
    error::{Error, Result},
};

fn run() -> Result<()> {
    let api = device::enumerate()?;
    let mut matches = api.device_list().filter(|d| {
        d.vendor_id() == 0x1b1c
            && d.product_id() == 0x1b75
            && d.usage_page() == 0xffc2
            && d.usage() == 4
            && matches!(d.bus_type(), hidapi::BusType::Usb)
    });
    let info = matches.next().ok_or(Error::NotFound)?;
    if matches.next().is_some() {
        return Err(Error::Ambiguous);
    }
    let h = info.open_device(&api).map_err(|_| Error::Access)?;
    let descriptor = device::read_descriptor(&h)?;
    for r in &descriptor.reports {
        println!("report {:?} id={} bytes={}", r.kind, r.id, r.bytes());
    }
    if !descriptor
        .reports
        .iter()
        .any(|r| r.kind == Kind::Feature && r.id == 0 && r.bytes() == 64)
    {
        return Err(Error::Safety("expected unnumbered feature64".into()));
    }
    for sub in [
        None,
        Some(2),
        Some(5),
        Some(0xd0),
        Some(0xd1),
        Some(0xd2),
        Some(0xd3),
        Some(0xd4),
        Some(0xd5),
    ] {
        let mut tx = [0u8; 65];
        tx[1] = 0x0e;
        tx[2] = if sub.is_some() { 0x13 } else { 1 };
        tx[3] = sub.unwrap_or(0);
        h.send_feature_report(&tx)
            .map_err(|_| Error::Disconnected)?;
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut rx = [0u8; 65];
        let n = h
            .get_feature_report(&mut rx)
            .map_err(|_| Error::Disconnected)?;
        if n != 65 || rx[0] != 0 || rx[1..5] != tx[1..5] {
            return Err(Error::Protocol(format!(
                "reply size={n}, command echo mismatch"
            )));
        }
        if sub.is_none() {
            let p = &rx[1..];
            if p[12..16] != [0x1c, 0x1b, 0x75, 0x1b] {
                return Err(Error::Protocol("identity mismatch".into()));
            }
            println!(
                "identity fw={:02x}.{:02x} boot={:02x}.{:02x} vid=1b1c pid=1b75",
                p[9], p[8], p[11], p[10]
            );
            println!(
                "lighting capability header={:02x?} mask={:02x}",
                &p[21..25],
                p[25]
            );
        } else {
            println!(
                "reply {}",
                rx[1..13]
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
