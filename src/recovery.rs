//! Settings backup, never firmware or raw commands.
use crate::{error::Result, portable::PortableProfile};
#[derive(Clone, Debug)]
pub struct RecoveryInfo {
    pub dpi: u16,
    pub rgb: Option<[u8; 3]>,
    pub file: Option<String>,
    pub can_restore: bool,
    pub zone_effects: Option<usize>,
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

pub fn save_logitech_settings(dpi: u16, snapshot: &crate::rgb8070::Snapshot) -> Result<String> {
    let text = serde_json::to_string_pretty(&serde_json::json!({
        "format": "unidevice-logitech8070-settings", "version": 1,
        "dpi": dpi, "lighting": snapshot, "restore": "explicit_same_session_only",
        "firmware_image": false, "portable_rgb": snapshot.portable_color(),
    }))
    .map_err(|_| crate::error::Error::Io("cannot serialize zone settings".into()))?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("before-8070-{stamp}");
    crate::storage::save_new(&crate::storage::path("backups", &name)?, &text)?;
    Ok(format!("backups/{name}.json"))
}
