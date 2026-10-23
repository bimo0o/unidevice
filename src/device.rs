use crate::error::{Error, Result};
use hidapi::{DeviceInfo, HidApi, HidDevice};

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Model {
    Auto,
    Superlight,
    Harpoon,
    Gembird,
}

impl Model {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "auto" => Ok(Self::Auto),
            "superlight" => Ok(Self::Superlight),
            "harpoon" => Ok(Self::Harpoon),
            "gembird" => Ok(Self::Gembird),
            _ => Err(Error::Invalid(
                "model must be auto, superlight, harpoon or gembird".into(),
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Auto => "Auto capability discovery (no model whitelist)",
            Self::Superlight => "Logitech G PRO X SUPERLIGHT 1 (wired)",
            Self::Harpoon => "Corsair Harpoon RGB PRO (wired)",
            Self::Gembird => "Gembird (unidentified keyboard)",
        }
    }

    pub fn matches(self, vid: u16, pid: u16) -> bool {
        match self {
            Self::Auto => vid == 0x046d,
            Self::Superlight => (vid, pid) == (0x046d, 0xc094),
            Self::Harpoon => (vid, pid) == (0x1b1c, 0x1b75),
            Self::Gembird => false,
        }
    }
}

pub fn label(vid: u16, pid: u16) -> &'static str {
    for model in [Model::Superlight, Model::Harpoon] {
        if model.matches(vid, pid) {
            return model.name();
        }
    }
    if (vid, pid) == (0x046d, 0x4093) {
        return "Superlight logical wireless identity (routing unsupported)";
    }
    "Unknown HID (identify using printed model and VID/PID)"
}

pub fn enumerate() -> Result<HidApi> {
    HidApi::new().map_err(|_| Error::Access)
}

pub fn list() -> Result<()> {
    let api = enumerate()?;
    println!("INDEX  VID:PID    IFACE  USAGE PAGE:USAGE  MODEL");
    let mut count = 0;
    for (index, d) in api.device_list().enumerate() {
        println!(
            "{index:>5}  {:04X}:{:04X}  {:>5}  {:04X}:{:04X}        {}",
            d.vendor_id(),
            d.product_id(),
            d.interface_number(),
            d.usage_page(),
            d.usage(),
            label(d.vendor_id(), d.product_id())
        );
        count += 1;
    }
    println!("{count} HID collections. Indices are temporary; no serials or paths printed.");
    println!("Gembird cannot be identified by brand alone. No control reports were sent.");
    Ok(())
}

pub fn select(api: &HidApi, model: Model, index: Option<usize>) -> Result<&DeviceInfo> {
    if !matches!(model, Model::Superlight | Model::Auto) {
        return Err(Error::Unsupported(
            "this model has no enabled hardware transport".into(),
        ));
    }
    let eligible = |d: &&DeviceInfo| {
        model.matches(d.vendor_id(), d.product_id())
            && d.usage_page() == 0xff00
            && matches!(d.bus_type(), hidapi::BusType::Usb)
    };
    if let Some(index) = index {
        let d = api.device_list().nth(index).ok_or(Error::NotFound)?;
        if !eligible(&d) {
            return Err(Error::Invalid(
                "selected collection is not an eligible Logitech vendor collection".into(),
            ));
        }
        return Ok(d);
    }
    let mut devices = api.device_list().filter(eligible);
    let first = devices.next().ok_or(Error::NotFound)?;
    if devices.next().is_some() {
        return Err(Error::Ambiguous);
    }
    Ok(first)
}

pub fn open(model: Model, index: Option<usize>) -> Result<HidDevice> {
    let api = enumerate()?;
    let device = select(&api, model, index)?
        .open_device(&api)
        .map_err(|_| Error::Access)?;
    let descriptor = read_descriptor(&device)?;
    if !descriptor.hidpp_framing() {
        return Err(Error::Unsupported(
            "descriptor does not advertise accepted HID++ input/output framing".into(),
        ));
    }
    Ok(device)
}

pub fn read_descriptor(device: &HidDevice) -> Result<crate::descriptor::Descriptor> {
    let mut bytes = [0; 4096];
    let len = device.get_report_descriptor(&mut bytes).map_err(|_| {
        Error::Unsupported("cannot read HID descriptor; no speculative probing".into())
    })?;
    crate::descriptor::Descriptor::parse(&bytes[..len])
}

pub fn inspect(index: usize) -> Result<()> {
    let api = enumerate()?;
    let d = api.device_list().nth(index).ok_or(Error::NotFound)?;
    println!(
        "{:04X}:{:04X} | {}",
        d.vendor_id(),
        d.product_id(),
        label(d.vendor_id(), d.product_id())
    );
    let descriptor = read_descriptor(&d.open_device(&api).map_err(|_| Error::Access)?)?;
    for report in &descriptor.reports {
        println!(
            "{:?} report {:02X}: {} bytes",
            report.kind,
            report.id,
            report.bytes()
        );
    }
    if d.vendor_id() == 0x046d && d.usage_page() == 0xff00 && descriptor.hidpp_framing() {
        println!(
            "Adapter candidate: HID++2. No PID whitelist needed. Runtime feature probe required."
        );
    } else if descriptor.usage_pages.contains(&0x59) {
        println!("Lighting usage detected; standard lighting adapter not implemented yet. Control disabled.");
    } else {
        if let Some(hint) = crate::protocols::hint(d.vendor_id(), d.product_id()) {
            println!("{hint}");
        }
        println!(
            "No implemented control adapter. Descriptor alone does not specify vendor commands."
        );
    }
    println!("Descriptor inspection only. No vendor control reports sent. No unique IDs printed.");
    Ok(())
}

pub fn scan(experimental: bool) -> Result<()> {
    let api = enumerate()?;
    println!("ADAPTIVE SCAN | no model whitelist | writes disabled");
    let mut count = 0;
    for (index, d) in api.device_list().enumerate() {
        count += 1;
        println!(
            "\n[{index}] {:04X}:{:04X} | {}",
            d.vendor_id(),
            d.product_id(),
            label(d.vendor_id(), d.product_id())
        );
        let inspected = (|| -> Result<()> {
            let device = d.open_device(&api).map_err(|_| Error::Access)?;
            let descriptor = read_descriptor(&device)?;
            if d.vendor_id() == 0x046d && d.usage_page() == 0xff00 && descriptor.hidpp_framing() {
                if !experimental {
                    println!(
                        "HID++ adapter candidate; use scan --experimental to query capabilities."
                    );
                } else {
                    let mut session = crate::hidpp::Session::connect(
                        crate::transport::NativeTransport::new(device)?,
                    )?;
                    let current = session.read_dpi()?;
                    let values = session.supported_dpi()?;
                    println!(
                        "DPI control discovered: current {current}; {} advertised values.",
                        values.len()
                    );
                    println!("No PID entry required. Hardware behavior remains UNVERIFIED.");
                }
            } else if descriptor.usage_pages.contains(&0x59) {
                println!(
                    "Lighting usage found; lighting adapter not implemented. Control disabled."
                );
            } else {
                println!(
                    "{} report layouts discovered; no compatible control adapter yet.",
                    descriptor.reports.len()
                );
            }
            Ok(())
        })();
        if let Err(e) = inspected {
            println!("Control unavailable: {e}");
        }
    }
    println!("\n{count} collections scanned. No settings changed. No unique identifiers printed.");
    Ok(())
}
