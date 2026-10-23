use crate::error::{Error, Result};
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Adapter {
    Hidpp2,
    Harpoon313,
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pack {
    pub version: u8,
    pub id: String,
    pub name: String,
    pub adapter: Adapter,
    pub vid: u16,
    pub pids: Vec<u16>,
    pub max_dpi: Option<u16>,
    pub evidence: String,
    pub sources: Vec<String>,
}
impl Pack {
    pub fn validate(&self) -> Result<()> {
        let identity = match self.adapter {
            Adapter::Hidpp2 => self.vid == 0x046d,
            Adapter::Harpoon313 => self.vid == 0x1b1c && self.pids == [0x1b75],
        };
        if self.version != 1
            || !identity
            || self.id.is_empty()
            || self.id.len() > 48
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
            || self.name.is_empty()
            || self.name.chars().count() > 72
            || self.name.chars().any(char::is_control)
            || self.pids.is_empty()
            || self.pids.len() > 64
            || self.pids.contains(&0)
            || self.max_dpi.is_some_and(|d| {
                !(100..=if self.adapter == Adapter::Hidpp2 {
                    25600
                } else {
                    12000
                })
                    .contains(&d)
            })
            || self.evidence.len() > 1024
            || self.evidence.chars().any(char::is_control)
            || self.sources.len() > 8
            || self.sources.iter().any(|s| {
                s.len() > 240 || !s.starts_with("https://") || s.chars().any(char::is_control)
            })
        {
            return Err(Error::Invalid(
                "compatibility pack violates schema or immutable adapter policy".into(),
            ));
        }
        Ok(())
    }
    pub fn from_json(text: &str) -> Result<Self> {
        if text.len() > 16384 {
            return Err(Error::Invalid("pack exceeds 16KiB".into()));
        }
        let p: Self = serde_json::from_str(text)
            .map_err(|_| Error::Invalid("compatibility pack JSON".into()))?;
        p.validate()?;
        Ok(p)
    }
    pub fn json(&self) -> Result<String> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|_| Error::Io("pack serialization".into()))
    }
    pub fn matches(&self, vid: u16, pid: u16) -> bool {
        self.vid == vid && self.pids.contains(&pid)
    }
    pub fn install(text: &str) -> Result<String> {
        let pack = Self::from_json(text)?;
        crate::storage::save_new(
            &crate::storage::path("compatibility", &pack.id)?,
            &pack.json()?,
        )?;
        Ok(pack.name)
    }
}
pub fn installed() -> Vec<Pack> {
    let Ok(root) = crate::storage::root() else {
        return vec![];
    };
    let Ok(entries) = std::fs::read_dir(root.join("compatibility")) else {
        return vec![];
    };
    let mut files: Vec<_> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "json"))
        .take(65)
        .collect();
    files.sort();
    files
        .into_iter()
        .take(64)
        .filter_map(|p| {
            crate::storage::read(&p)
                .ok()
                .and_then(|s| Pack::from_json(&s).ok())
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Pack {
        Pack {
            version: 1,
            id: "custom-logitech".into(),
            name: "My mouse".into(),
            adapter: Adapter::Hidpp2,
            vid: 0x046d,
            pids: vec![0x1234],
            max_dpi: Some(6400),
            evidence: "User observation; unverified".into(),
            sources: vec![],
        }
    }
    #[test]
    fn packages_label_known_families_without_unlocking_commands() {
        let p = sample();
        assert!(p.matches(0x046d, 0x1234));
        assert!(!p.matches(0x1b1c, 0x1234));
        assert!(Pack::from_json(&p.json().unwrap()).is_ok());
        let mut p = sample();
        p.vid = 0x9999;
        assert!(p.validate().is_err());
        let mut p = sample();
        p.max_dpi = Some(30000);
        assert!(p.validate().is_err());
        let mut p = sample();
        p.id = "../override".into();
        assert!(p.validate().is_err());
        let p = sample();
        assert!(Pack::from_json(
            &p.json()
                .unwrap()
                .replace("\"version\": 1", "\"reports\": [7,2], \"version\": 1")
        )
        .is_err());
    }
}
