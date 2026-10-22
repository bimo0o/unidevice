use crate::controller::{rgb_payload, validate_dpi};
use crate::device::Model;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub version: u8,
    pub model: Model,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rgb: Option<[u8; 3]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zone: Option<u8>,
}

impl Profile {
    pub fn validate(&self) -> Result<()> {
        if self.version != 1 {
            return Err(Error::Invalid("unsupported profile version".into()));
        }
        match (self.dpi, self.rgb, self.zone) {
            (Some(dpi), None, None) => validate_dpi(self.model, dpi),
            (None, Some(rgb), Some(zone)) => rgb_payload(self.model, rgb, zone).map(|_| ()),
            _ => Err(Error::Invalid(
                "profile must contain one supported setting; RGB needs an explicit zone".into(),
            )),
        }
    }
    pub fn from_json(json: &str) -> Result<Self> {
        if json.len() > 4096 {
            return Err(Error::Invalid("profile exceeds 4 KiB".into()));
        }
        let p: Self =
            serde_json::from_str(json).map_err(|_| Error::Invalid("profile JSON schema".into()))?;
        p.validate()?;
        Ok(p)
    }
    pub fn load(path: &Path) -> Result<Self> {
        let file =
            std::fs::File::open(path).map_err(|_| Error::Io("cannot open profile".into()))?;
        let mut json = String::new();
        file.take(4097)
            .read_to_string(&mut json)
            .map_err(|_| Error::Io("cannot read UTF-8 profile".into()))?;
        Self::from_json(&json)
    }
    pub fn json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|_| Error::Io("cannot serialize profile".into()))
    }
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = self.json()?;
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|_| {
                Error::Io("cannot create profile (existing files are never overwritten)".into())
            })?;
        file.write_all(json.as_bytes())
            .and_then(|_| file.write_all(b"\n"))
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::Io("cannot finish profile write".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_roundtrip_has_no_device_identifier() {
        let p = Profile {
            version: 1,
            model: Model::Superlight,
            dpi: Some(800),
            rgb: None,
            zone: None,
        };
        let text = p.json().unwrap();
        assert_eq!(Profile::from_json(&text), Ok(p));
        assert!(!text.contains("serial"));
    }
    #[test]
    fn malformed_cross_model_and_future_profiles_are_rejected() {
        for text in [
            r#"{"version":2,"model":"superlight","dpi":800}"#,
            r#"{"version":1,"model":"harpoon","dpi":800}"#,
            r#"{"version":1,"model":"gembird","rgb":[1,2,3],"zone":0}"#,
            r#"{"version":1,"model":"superlight","dpi":800,"serial":"private"}"#,
            r#"{"version":1,"model":"harpoon","rgb":[1,2,3]}"#,
            r#"{"version":1,"model":"superlight","dpi":800,"dpi":1600}"#,
            r#"{"version":1,"model":"superlight","dpi":0}"#,
            "{}",
            "[]",
            "not-json",
        ] {
            assert!(Profile::from_json(text).is_err(), "{text}");
        }
        assert!(Profile::from_json(&" ".repeat(4097)).is_err());
    }
}
