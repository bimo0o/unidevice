use unidevice::error::Result;
fn run() -> Result<()> {
    let mut s = unidevice::corsair::open_harpoon()?;
    let before = s.state()?;
    println!("before {before:?}");
    println!("blue readback {:?}", s.set_stage_rgb([0, 0, 255])?);
    std::thread::sleep(std::time::Duration::from_secs(8));
    println!("restored {:?}", s.set_stage_rgb(before.rgb)?);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
