use crate::device::Model;
use crate::error::{Error, Result};
use crate::hidpp::Session;
use crate::transport::Transport;

pub trait DeviceController {
    fn set_dpi(&mut self, _: u16) -> Result<u16> {
        Err(Error::Unsupported("DPI for this model".into()))
    }
    fn set_rgb(&mut self, _: [u8; 3], _: u8) -> Result<()> {
        Err(Error::Unsupported(
            "RGB hardware transport is not specified".into(),
        ))
    }
}

pub struct Logitech<T>(pub Session<T>);
impl<T: Transport> DeviceController for Logitech<T> {
    fn set_dpi(&mut self, dpi: u16) -> Result<u16> {
        self.0.set_dpi(dpi)
    }
}
pub struct Corsair;
impl DeviceController for Corsair {}
pub struct UnknownKeyboard;
impl DeviceController for UnknownKeyboard {}

pub fn validate_dpi(model: Model, value: u16) -> Result<()> {
    if !matches!(model, Model::Superlight | Model::Auto) {
        return Err(Error::Unsupported(
            "DPI requires the adaptive HID++ adapter".into(),
        ));
    }
    if !(100..=25600).contains(&value) {
        return Err(Error::Invalid(
            "experimental DPI policy is 100..25600; device capabilities further restrict it".into(),
        ));
    }
    Ok(())
}

pub fn rgb_payload(model: Model, color: [u8; 3], zone: u8) -> Result<Vec<u8>> {
    if model != Model::Harpoon {
        return Err(Error::Unsupported(
            "RGB preview is available only for Harpoon".into(),
        ));
    }
    if zone > 15 {
        return Err(Error::Invalid(
            "preview zone must be 0..15; actual mapping is unknown".into(),
        ));
    }
    let mut payload = vec![0; 64];
    payload[..8].copy_from_slice(&[7, 0x22, 1, 1, zone, color[0], color[1], color[2]]);
    Ok(payload)
}

pub fn parse_color(value: &str) -> Result<[u8; 3]> {
    let value = value.strip_prefix('#').unwrap_or(value);
    if value.len() != 6 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Invalid(
            "color must be six hexadecimal digits, e.g. FF8800".into(),
        ));
    }
    Ok([
        u8::from_str_radix(&value[0..2], 16).unwrap(),
        u8::from_str_radix(&value[2..4], 16).unwrap(),
        u8::from_str_radix(&value[4..6], 16).unwrap(),
    ])
}

pub fn require_hardware(experimental: bool) -> Result<()> {
    if experimental {
        Ok(())
    } else {
        Err(Error::Unverified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_model_routes_only_its_own_operation() {
        assert!(Model::Auto.matches(0x046d, 0x1234));
        assert!(!Model::Superlight.matches(0x046d, 0x1234));
        assert!(!Model::Auto.matches(0x1234, 0x1234));
        assert!(validate_dpi(Model::Auto, 800).is_ok());
        assert!(validate_dpi(Model::Harpoon, 800).is_err());
        assert!(validate_dpi(Model::Gembird, 800).is_err());
        assert!(rgb_payload(Model::Superlight, [1, 2, 3], 0).is_err());
        assert!(rgb_payload(Model::Gembird, [1, 2, 3], 0).is_err());
        assert!(Corsair.set_dpi(800).is_err());
        assert!(Corsair.set_rgb([1, 2, 3], 0).is_err());
        assert!(UnknownKeyboard.set_rgb([1, 2, 3], 0).is_err());
    }
    #[test]
    fn corsair_fixture_is_payload_without_invented_report_id() {
        let p = rgb_payload(Model::Harpoon, [0xff, 0x40, 0x80], 2).unwrap();
        assert_eq!(&p[..8], &[7, 0x22, 1, 1, 2, 0xff, 0x40, 0x80]);
        assert_eq!(p.len(), 64);
        assert!(p[8..].iter().all(|b| *b == 0));
        assert!(rgb_payload(Model::Harpoon, [0, 0, 0], 16).is_err());
    }
    #[test]
    fn invalid_colors_and_default_hardware_access_are_rejected() {
        assert_eq!(parse_color("#ff4080"), Ok([255, 64, 128]));
        for s in ["", "f00", "GGGGGG", "💡xx", "ff408000"] {
            assert!(parse_color(s).is_err());
        }
        assert_eq!(require_hardware(false), Err(Error::Unverified));
    }
}
