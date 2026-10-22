use unidevice::device;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("list") if args.len() == 1 => device::list(),
        _ => {
            println!("UniDevice 0.1.0 | AI-written pet project under author supervision\nCommands: list\nAll hardware control is unverified and disabled by default.");
            Ok(())
        }
    };
    if let Err(error) = result {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
