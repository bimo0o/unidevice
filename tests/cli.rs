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
