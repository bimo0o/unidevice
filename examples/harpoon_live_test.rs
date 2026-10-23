use unidevice::error::Result;
fn run() -> Result<()> {
    let mut session = unidevice::corsair::open_harpoon()?;
    let before = session.state()?;
    println!("before {before:?}");
    if before.x != before.y {
        return Err(unidevice::error::Error::Safety(
            "asymmetric DPI not restored by this lab test".into(),
        ));
    }
    let target = if before.x == 800 { 1000 } else { 800 };
    let after = session.set_dpi(target)?;
    println!("test DPI readback {after:?}");
    std::thread::sleep(std::time::Duration::from_secs(3));
    // This explicit lab restoration runs only after a successful, verified test.
    let restored = session.set_dpi(before.x)?;
    println!("restored {restored:?}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
