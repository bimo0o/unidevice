//! Settings backup, never firmware or raw commands.
use crate::{error::Result, portable::PortableProfile};
#[derive(Clone, Debug)]
pub struct RecoveryInfo {
    pub dpi: u16,
    pub rgb: Option<[u8; 3]>,
    pub file: Option<String>,
    pub can_restore: bool,
}
pub fn save_settings(dpi: Option<u16>, rgb: Option<[u8; 3]>) -> Result<String> {
    let profile = PortableProfile {
        version: 2,
        dpi,
        rgb,
        effect: None,
        feel: None,
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("before-{stamp}");
    crate::storage::save_new(&crate::storage::path("backups", &name)?, &profile.json()?)?;
    Ok(format!("backups/{name}.json"))
}
