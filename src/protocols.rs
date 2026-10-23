//! Recognition metadata has no access to device I/O or adapter permissions.
//! VID/PID matches are hints, including logical receiver IDs from upstream data.
use std::collections::BTreeSet;
use std::sync::OnceLock;

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub vid: u16,
    pub pid: u16,
    pub name: String,
    pub family: String,
    pub source_record: String,
    pub source_url: String,
    pub identity_kind: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Index {
    schema_version: u8,
    checked_on: String,
    purpose: String,
    identities: Vec<Identity>,
}

fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| {
        // Trusted compile-time artifact, not an externally supplied permission file.
        let value: Index =
            serde_json::from_str(include_str!("../research/mouse-protocol-index.json"))
                .expect("invalid bundled recognition index");
        assert_eq!(value.schema_version, 1);
        assert_eq!(value.purpose, "recognition_only_never_authorizes_io");
        value
    })
}

pub fn lookup(vid: u16, pid: u16) -> Vec<&'static Identity> {
    index()
        .identities
        .iter()
        .filter(|r| r.vid == vid && r.pid == pid)
        .collect()
}

pub fn hint(vid: u16, pid: u16) -> Option<String> {
    let identities = lookup(vid, pid);
    if identities.is_empty() {
        return None;
    }
    let families: BTreeSet<_> = identities.iter().map(|r| r.family.as_str()).collect();
    Some(format!(
        "Есть сведения о семействе: {}. Это совпадение по ID; проверенного управления пока нет.",
        families.into_iter().collect::<Vec<_>>().join(", ")
    ))
}

pub fn summary() -> serde_json::Value {
    let data = index();
    let pairs: BTreeSet<_> = data.identities.iter().map(|r| (r.vid, r.pid)).collect();
    let families: BTreeSet<_> = data.identities.iter().map(|r| &r.family).collect();
    serde_json::json!({
        "scope": "bundled research inventory; recognition only",
        "checked_on": data.checked_on,
        "identity_source_records": data.identities.len(),
        "distinct_vid_pid_pairs": pairs.len(),
        "families": families,
        "goal": {"target_percent": 99, "population": "gaming mice configurable in manufacturer software", "required": "DPI and lighting where present", "transports": ["USB", "USB receiver"], "os": ["Windows", "Linux"]},
        "market_coverage_probability": null,
        "representative_sample_available": false,
        "native": {"hidpp2": "conditional wired DPI discovery: 2201 or experimental 2202 v0; RGB not implemented", "corsair": "Harpoon RGB PRO 1B75 firmware 3.13 DPI and active-stage RGB"},
        "physical_verified_model_firmware_variants": 1,
        "razer_experimental_read_only_profiles": crate::razer::profile_count(),
        "razer_dpi_and_rgb_setters_available": false,
        "recognition_grants_write_permission": false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_hint_has_auditable_provenance_and_no_packet_fields() {
        let data = index();
        assert!(data.identities.len() > 300);
        for r in &data.identities {
            assert_ne!(r.vid, 0);
            assert_ne!(r.pid, 0);
            assert!(!r.name.is_empty());
            assert!(!r.source_record.is_empty());
            assert!(r.source_url.starts_with("https://"));
            assert!(!r.identity_kind.is_empty());
        }
    }

    #[test]
    fn known_and_unknown_ids_do_not_become_control_authorization() {
        let razer = lookup(0x1532, 0x0084);
        assert!(razer.iter().any(|r| r.family == "razer_model_specific"));
        assert!(hint(0x1532, 0x0084)
            .unwrap()
            .contains("проверенного управления пока нет"));
        assert!(lookup(0xffff, 0xffff).is_empty());
        assert!(hint(0xffff, 0xffff).is_none());
        let result = summary();
        assert!(result["market_coverage_probability"].is_null());
        assert_eq!(result["recognition_grants_write_permission"], false);
    }
}
