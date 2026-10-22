use crate::error::{Error, Result};
use hidapi::{DeviceInfo, HidApi, HidDevice};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    Superlight,
    Harpoon,
    Gembird,
}

impl Model {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "superlight" => Ok(Self::Superlight),
            "harpoon" => Ok(Self::Harpoon),
            "gembird" => Ok(Self::Gembird),
            _ => Err(Error::Invalid(
                "model must be superlight, harpoon or gembird".into(),
            )),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Superlight => "Logitech G PRO X SUPERLIGHT 1 (wired)",
            Self::Harpoon => "Corsair Harpoon RGB PRO (wired)",
            Self::Gembird => "Gembird (unidentified keyboard)",
        }
    }

    pub fn matches(self, vid: u16, pid: u16) -> bool {
        match self {
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

pub fn select<'a>(api: &'a HidApi, model: Model, index: Option<usize>) -> Result<&'a DeviceInfo> {
    if model != Model::Superlight {
        return Err(Error::Unsupported(
            "this model has no enabled hardware transport".into(),
        ));
    }
    let eligible =
        |d: &&DeviceInfo| model.matches(d.vendor_id(), d.product_id()) && d.usage_page() == 0xff00;
    if let Some(index) = index {
        let d = api.device_list().nth(index).ok_or(Error::NotFound)?;
        if !eligible(&d) {
            return Err(Error::Invalid(
                "selected collection is not a wired Superlight vendor collection".into(),
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
    select(&api, model, index)?
        .open_device(&api)
        .map_err(|_| Error::Access)
}
