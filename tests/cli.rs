use std::process::{Command, Output};
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_unidevice"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn protocol_inventory_does_not_report_inventory_as_probability() {
    let output = run(&["protocols"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("\"market_coverage_probability\": null"));
    assert!(text.contains("\"recognition_grants_write_permission\": false"));
    assert!(text.contains("gaming mice configurable in manufacturer software"));
}

#[test]
fn model_summary_separates_experimental_protocols_from_hardware_evidence() {
    let output = run(&["models"]);
    assert!(output.status.success());
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("0x2202 v0"));
    assert!(text.contains("1B1C:1B75 firmware 3.13"));
    assert!(text.contains("no DPI/RGB setters"));
    assert!(!text.contains("All hardware adapters remain UNVERIFIED"));
}

#[test]
fn razer_simulation_is_explicit_and_rejects_mixed_hardware_flags() {
    let output = run(&["razer-probe", "--demo"]);
    assert!(output.status.success());
    // The CLI banner precedes the JSON document.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let start = stdout.find('{').unwrap();
    let observed: serde_json::Value = serde_json::from_str(&stdout[start..]).unwrap();
    assert_eq!(observed["dpi_x"], 800);
    assert_eq!(observed["dpi_y"], 800);
    assert_eq!(observed["settings_modified"], false);
    assert_eq!(observed["setters_available"], false);
    assert_eq!(observed["hardware_verified"], false);
    assert_eq!(observed["transport"], "simulation_no_device_access");
    for args in [
        vec!["razer-probe"],
        vec!["razer-probe", "--device", "0"],
        vec!["razer-probe", "--demo", "--device", "0"],
        vec!["razer-probe", "--demo", "--experimental"],
        vec!["razer-probe", "--experimental"],
        vec!["razer-probe", "--experimental", "--device", "-1"],
        vec!["razer-probe", "--demo", "--apply"],
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "{args:?}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(!error.contains("device not found"), "{args:?}: {error}");
        assert!(!error.contains("cannot open HID"), "{args:?}: {error}");
    }
}

#[test]
fn demo_and_preview_are_available_without_hardware() {
    for args in [
        vec!["demo"],
        vec!["dpi", "--value", "800"],
        vec!["dpi", "--model", "superlight", "--value", "800"],
        vec![
            "rgb", "--model", "harpoon", "--color", "FF8800", "--zone", "0",
        ],
        vec!["profile", "apply", "profiles/superlight-800.json"],
        vec!["profile", "apply", "profiles/harpoon-blue-preview.json"],
    ] {
        let output = run(&args);
        assert!(
            output.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("OFFLINE") || args[0] == "demo");
    }
}

#[test]
fn invalid_requests_fail_before_opening_devices() {
    for args in [
        vec!["dpi", "--value", "800", "--apply"],
        vec!["dpi", "--value", "800", "--device", "0"],
        vec![
            "dpi",
            "--model",
            "harpoon",
            "--value",
            "800",
            "--apply",
            "--experimental",
        ],
        vec![
            "rgb", "--model", "harpoon", "--color", "FF8800", "--zone", "0", "--apply",
        ],
        vec![
            "rgb", "--model", "gembird", "--color", "FF8800", "--zone", "0",
        ],
        vec!["status"],
        vec![
            "profile",
            "apply",
            "profiles/harpoon-blue-preview.json",
            "--apply",
            "--experimental",
        ],
        vec!["dpi", "--value", "800", "--value", "1600"],
        vec!["dpi", "--value", "-1"],
        vec!["list", "--experimental"],
        vec!["unknown"],
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "{args:?}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            !error.contains("device not found"),
            "should fail before HID access: {args:?}"
        );
        assert!(
            !error.contains("cannot open HID"),
            "should fail before HID access: {args:?}"
        );
    }
}

#[test]
fn profile_save_roundtrip_refuses_overwrite() {
    let file = std::env::temp_dir().join(format!("unidevice-test-{}.json", std::process::id()));
    let path = file.to_str().unwrap();
    let args = ["profile", "save", path, "--model", "auto", "--dpi", "800"];
    assert!(run(&args).status.success());
    assert!(!run(&args).status.success());
    let output = run(&["profile", "show", path]);
    std::fs::remove_file(&file).unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("800"));
}

#[test]
fn console_exits_cleanly_on_closed_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_unidevice"))
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Pet project"));
}
