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
    #[serde(default = "first_revision")]
    pub revision: u32,
    pub id: String,
    pub name: String,
    pub adapter: Adapter,
    pub vid: u16,
    pub pids: Vec<u16>,
    pub max_dpi: Option<u16>,
    pub evidence: String,
    pub sources: Vec<String>,
}
fn first_revision() -> u32 {
    1
}
impl Pack {
    pub fn validate(&self) -> Result<()> {
        let identity = match self.adapter {
            Adapter::Hidpp2 => self.vid == 0x046d,
            Adapter::Harpoon313 => self.vid == 0x1b1c && self.pids == [0x1b75],
        };
        if self.version != 1
            || self.revision == 0
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
            || self
                .evidence
                .chars()
                .any(|c| c.is_control() && !matches!(c, '\n' | '\t'))
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
        pack.install_at(&crate::storage::path("compatibility", &pack.id)?)?;
        Ok(pack.name)
    }
    fn install_at(&self, path: &std::path::Path) -> Result<()> {
        use std::io::Write;
        self.validate()?;
        let json = self.json()?;
        if !path.exists() {
            return crate::storage::save_new(path, &json);
        }
        if std::fs::symlink_metadata(path)
            .map_err(|_| Error::Io("pack metadata".into()))?
            .file_type()
            .is_symlink()
        {
            return Err(Error::Safety("package symlink replacement blocked".into()));
        }
        let previous = crate::storage::read(path)?;
        let old = Self::from_json(&previous)?;
        if old.id != self.id || self.revision <= old.revision {
            return Err(Error::Invalid(
                "package update needs the same ID and a higher revision".into(),
            ));
        }
        let backup = path.with_extension(format!("r{}.json.backup", old.revision));
        crate::storage::save_new(&backup, &previous)?;
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(path)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()
        })();
        if result.is_err() {
            let _ = std::fs::write(path, previous);
            return Err(Error::Io(
                "package update failed; previous revision backed up".into(),
            ));
        }
        Ok(())
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
            revision: 1,
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
    fn shipped_pack_and_updates_keep_previous_revision_recoverable() {
        Pack::from_json(include_str!("../compatibility/harpoon-rgb-pro.json")).unwrap();
        let root = std::env::temp_dir().join(format!(
            "unidevice-pack-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = root.join("custom-logitech.json");
        let mut p = sample();
        p.install_at(&path).unwrap();
        assert!(p.install_at(&path).is_err());
        p.revision = 2;
        p.name = "Updated mouse".into();
        p.install_at(&path).unwrap();
        assert_eq!(
            Pack::from_json(&crate::storage::read(&path).unwrap())
                .unwrap()
                .revision,
            2
        );
        let backup = path.with_extension("r1.json.backup");
        assert_eq!(
            Pack::from_json(&crate::storage::read(&backup).unwrap())
                .unwrap()
                .revision,
            1
        );
        std::fs::remove_file(path).unwrap();
        std::fs::remove_file(backup).unwrap();
        std::fs::remove_dir(root).unwrap();
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
