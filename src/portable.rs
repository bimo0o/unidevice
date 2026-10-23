use crate::{
    effects::Effect,
    error::{Error, Result},
    sensitivity::Feel,
};
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableProfile {
    pub version: u8,
    pub dpi: Option<u16>,
    pub rgb: Option<[u8; 3]>,
    pub effect: Option<Effect>,
    pub feel: Option<Feel>,
}
impl PortableProfile {
    pub fn from_any_json(text: &str) -> Result<Self> {
        Self::from_json(text).or_else(|_| {
            let legacy = crate::profile::Profile::from_json(text)?;
            if legacy.rgb.is_some() || legacy.zone.is_some() {
                return Err(Error::Unsupported(
                    "legacy RGB zone cannot be silently mapped to a Studio stage color".into(),
                ));
            }
            Ok(Self {
                version: 2,
                dpi: legacy.dpi,
                rgb: None,
                effect: None,
                feel: None,
            })
        })
    }
    pub fn validate(&self) -> Result<()> {
        if self.version != 2
            || self.dpi.is_some_and(|v| !(100..=25600).contains(&v))
            || self.effect.as_ref().is_some_and(|e| !e.valid())
            || self.feel.as_ref().is_some_and(|f| !f.valid())
            || self.dpi.is_none()
                && self.rgb.is_none()
                && self.effect.is_none()
                && self.feel.is_none()
        {
            return Err(Error::Invalid(
                "portable profile schema or capability".into(),
            ));
        }
        Ok(())
    }
    pub fn from_json(text: &str) -> Result<Self> {
        if text.len() > 16384 {
            return Err(Error::Invalid("profile too large".into()));
        }
        let p: Self = serde_json::from_str(text)
            .map_err(|_| Error::Invalid("portable profile JSON".into()))?;
        p.validate()?;
        Ok(p)
    }
    pub fn json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|_| Error::Io("profile serialization".into()))
    }
}
pub fn nearest(values: &[u16], wanted: f64) -> Option<u16> {
    if !wanted.is_finite() {
        return None;
    }
    values.iter().copied().min_by(|a, b| {
        (f64::from(*a) - wanted)
            .abs()
            .total_cmp(&(f64::from(*b) - wanted).abs())
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn portable_profiles_reject_commands_and_do_not_silently_quantize() {
        PortableProfile::from_json(include_str!("../profiles/studio-starter.json")).unwrap();
        assert!(
            PortableProfile::from_any_json(r#"{"version":1,"model":"auto","dpi":800}"#).is_ok()
        );
        assert!(PortableProfile::from_any_json(
            r#"{"version":1,"model":"harpoon","rgb":[1,2,3],"zone":2}"#
        )
        .is_err());
        let text = r#"{"version":2,"dpi":1100,"rgb":[1,2,3],"effect":null,"feel":null}"#;
        assert_eq!(PortableProfile::from_json(text).unwrap().dpi, Some(1100));
        assert_eq!(nearest(&[800, 1200], 1100.0), Some(1200));
        assert!(
            PortableProfile::from_json(&text.replace("\"feel\":null", "\"report\":[7,2]")).is_err()
        );
        assert!(PortableProfile::from_json(&" ".repeat(16385)).is_err());
    }
}
