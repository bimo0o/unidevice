use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::path::Path;
use unidevice::controller::{
    parse_color, require_hardware, rgb_payload, validate_dpi, DeviceController, Logitech,
};
use unidevice::device::{self, Model};
use unidevice::error::{Error, Result};
use unidevice::hidpp::{demo_session, Session};
use unidevice::profile::Profile;
use unidevice::transport::NativeTransport;

const BANNER: &str = concat!(
    "UniDevice ",
    env!("CARGO_PKG_VERSION"),
    " | Pet project, fully AI-written under author supervision"
);
const HELP: &str = r#"
  unidevice                  Interactive console (no idle polling)
  unidevice demo             Simulated devices; no HID access
  unidevice list             Enumerate local HID without serials/paths
  unidevice scan             Automatically inspect HID and select adapter candidates
  unidevice scan --experimental
                            Also discover documented HID++ capabilities; no setters
  unidevice inspect --device N
                            Read descriptor; identify protocol adapter candidates
  unidevice models           Show support boundaries
  unidevice protocols        Offline protocol inventory and honest coverage status
  unidevice razer-probe --demo
                            Synthetic Razer read-only example; no device access
  unidevice razer-probe --device N --experimental
                            Known USB Razer firmware/current XY DPI queries; setters blocked
  unidevice dpi --value 800  Offline plan; auto capability discovery by default
  unidevice dpi --value 800 --apply --experimental --device N
                            Discover HID++ DPI on any eligible wired Logitech model
  unidevice status --experimental --device N
                            Discover capabilities and read current DPI
  unidevice rgb --model harpoon --color FF8800 --zone 0
                            Offline payload preview; zone mapping unverified
  unidevice profile save FILE --model auto --dpi 800
  unidevice profile save FILE --model harpoon --color FF8800 --zone 0
  unidevice profile show FILE
  unidevice profile apply FILE [--apply --experimental --device N]
                            Preview unless --apply; Corsair writes always blocked
  unidevice idle --seconds 30
                            Offline idle hold for external memory/CPU measurement

Optional --model superlight restricts DPI to known wired 046D:C094.
Default --model auto uses descriptor + runtime features, never a PID whitelist.
All hardware control is UNVERIFIED. Close vendor software before experimental use.
HID++: wired target only. Harpoon: preview only. Unknown protocols: inspect only.
No firmware, flash, reset, macros, remap, polling control or arbitrary reports.
No accounts, telemetry, serial/path logs, startup service or network calls.
"#;

#[derive(Default)]
struct Options {
    values: BTreeMap<String, String>,
    flags: BTreeSet<String>,
}
impl Options {
    fn parse(args: &[String], keys: &[&str], flags: &[&str]) -> Result<Self> {
        let mut result = Self::default();
        let mut i = 0;
        while i < args.len() {
            let key = args[i]
                .strip_prefix("--")
                .ok_or_else(|| Error::Invalid("expected --option".into()))?;
            if result.values.contains_key(key) || result.flags.contains(key) {
                return Err(Error::Invalid(format!("duplicate --{key}")));
            }
            if flags.contains(&key) {
                result.flags.insert(key.to_string());
            } else if keys.contains(&key) {
                i += 1;
                let value = args
                    .get(i)
                    .filter(|s| !s.starts_with("--"))
                    .ok_or_else(|| Error::Invalid(format!("missing --{key} value")))?;
                result.values.insert(key.to_string(), value.clone());
            } else {
                return Err(Error::Invalid(format!("unknown option --{key}")));
            }
            i += 1;
        }
        Ok(result)
    }
    fn value(&self, key: &str) -> Result<&str> {
        self.values
            .get(key)
            .map(String::as_str)
            .ok_or_else(|| Error::Invalid(format!("missing --{key}")))
    }
    fn number<T: std::str::FromStr>(&self, key: &str) -> Result<T> {
        self.value(key)?
            .parse()
            .map_err(|_| Error::Invalid(format!("--{key} must be an unsigned integer")))
    }
    fn index(&self) -> Result<Option<usize>> {
        self.values
            .get("device")
            .map(|s| {
                s.parse()
                    .map_err(|_| Error::Invalid("--device index".into()))
            })
            .transpose()
    }
    fn flag(&self, flag: &str) -> bool {
        self.flags.contains(flag)
    }
    fn model(&self) -> Result<Model> {
        self.values
            .get("model")
            .map(|s| Model::parse(s))
            .unwrap_or(Ok(Model::Auto))
    }
}

fn models() {
    println!("ADAPTER / MODEL     FUNCTION  STATUS\nauto (HID++2)      DPI       descriptor + dynamic feature discovery; no PID whitelist\nsuperlight         DPI       optional restriction to wired 046D:C094\nharpoon            RGB       offline preview; report ID/zones/init need capture\nharpoon            DPI       not implemented; write semantics need capture\nunknown/Gembird    inspect   descriptors and adapter candidates; control blocked\n\nAll hardware adapters remain UNVERIFIED. Unknown vendor payload semantics are not guessed.");
}

fn preview_rgb(model: Model, color: [u8; 3], zone: u8) -> Result<()> {
    let payload = rgb_payload(model, color, zone)?;
    println!(
        "OFFLINE PREVIEW | {} | #{:02X}{:02X}{:02X} | user-supplied zone {zone}",
        model.name(),
        color[0],
        color[1],
        color[2]
    );
    println!("64-byte NXP payload, not a HID report. Report ID, zone mapping and init unknown.");
    for line in payload.chunks(16) {
        println!(
            "{}",
            line.iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }
    println!("No device opened. No data sent. Corsair hardware writes are blocked.");
    Ok(())
}

fn dpi(model: Model, value: u16, o: &Options) -> Result<()> {
    validate_dpi(model, value)?;
    let index = o.index()?;
    if !o.flag("apply") {
        if o.flag("experimental") || index.is_some() {
            return Err(Error::Invalid(
                "hardware flags need --apply; remove them for offline preview".into(),
            ));
        }
        println!("OFFLINE PLAN | {} | DPI {value}", model.name());
        println!("Descriptor -> ping -> discover 2201 -> sensor count -> supported DPI -> set -> readback.");
        let [hi, lo] = value.to_be_bytes();
        println!("Set parameters: 00 {hi:02X} {lo:02X}. Feature index/range come from the device.");
        println!("No device opened. Review specs before using --apply --experimental.");
        return Ok(());
    }
    require_hardware(o.flag("experimental"))?;
    let mut controller = Logitech(Session::connect(NativeTransport::new(device::open(
        model, index,
    )?))?);
    let original = controller.0.read_dpi()?;
    let backup = unidevice::recovery::save_settings(Some(original), None)?;
    println!("Original DPI saved to {backup}; no automatic rollback.");
    let actual = controller.set_dpi(value)?;
    println!("DPI {actual} acknowledged and read back. Physical behavior remains UNVERIFIED.");
    Ok(())
}

fn demo() -> Result<()> {
    println!("{BANNER}\nDEMO / SYNTHETIC FIXTURES / NO HARDWARE ACCESS\n");
    let mut session = demo_session()?;
    println!(
        "[Unlisted HID++ mouse] Simulated DPI change: {}; readback matched.",
        session.set_dpi(800)?
    );
    println!(
        "Model/PID not used. Fixture feature index 7 is synthetic, never a hardware constant."
    );
    preview_rgb(Model::Harpoon, [0x42, 0xb8, 0xff], 0)?;
    println!("\n[Gembird] Unknown protocol -> control blocked. Use list, then inspect --device N.");
    Ok(())
}

fn profile(args: &[String]) -> Result<()> {
    if args.len() < 2 {
        return Err(Error::Invalid("profile needs save/show/apply FILE".into()));
    }
    let path = Path::new(&args[1]);
    match args[0].as_str() {
        "save" => {
            let o = Options::parse(&args[2..], &["model", "dpi", "color", "zone"], &[])?;
            let p = Profile {
                version: 1,
                model: o.model()?,
                dpi: o.values.get("dpi").map(|_| o.number("dpi")).transpose()?,
                rgb: o.values.get("color").map(|s| parse_color(s)).transpose()?,
                zone: o.values.get("zone").map(|_| o.number("zone")).transpose()?,
            };
            p.save(path)?;
            println!("Profile saved locally. No device accessed.");
            Ok(())
        }
        "show" if args.len() == 2 => {
            println!("{}", Profile::load(path)?.json()?);
            Ok(())
        }
        "apply" => {
            let o = Options::parse(&args[2..], &["device"], &["apply", "experimental"])?;
            let p = Profile::load(path)?;
            if let Some(value) = p.dpi {
                return dpi(p.model, value, &o);
            }
            if o.flag("apply") || o.flag("experimental") || o.index()?.is_some() {
                return Err(Error::Unsupported(
                    "Corsair profiles are offline previews only".into(),
                ));
            }
            preview_rgb(p.model, p.rgb.unwrap(), p.zone.unwrap())
        }
        _ => Err(Error::Invalid(
            "profile needs save/show/apply FILE; unexpected arguments".into(),
        )),
    }
}

fn menu() -> Result<()> {
    println!("{BANNER}\nHardware: UNVERIFIED. Writes off by default.");
    loop {
        println!(
            "\n1  Demo (no hardware)\n2  Auto-detect connected HID\n3  Support matrix\n4  Commands\n0  Exit"
        );
        print!("> ");
        io::stdout()
            .flush()
            .map_err(|_| Error::Io("console output".into()))?;
        let mut line = String::new();
        if io::stdin()
            .read_line(&mut line)
            .map_err(|_| Error::Io("console input".into()))?
            == 0
        {
            return Ok(());
        }
        let result = match line.trim() {
            "1" => demo(),
            "2" => device::scan(false),
            "3" => {
                models();
                Ok(())
            }
            "4" => {
                println!("{HELP}");
                Ok(())
            }
            "0" | "q" | "exit" => return Ok(()),
            _ => {
                println!("Choose 0..4.");
                Ok(())
            }
        };
        if let Err(e) = result {
            eprintln!("Error: {e}");
        }
    }
}

fn run(args: &[String]) -> Result<()> {
    let Some(command) = args.first() else {
        return menu();
    };
    match command.as_str() {
        "--help" | "-h" | "help" if args.len() == 1 => {
            println!("{BANNER}\n{HELP}");
            Ok(())
        }
        "--version" if args.len() == 1 => {
            println!("{BANNER}");
            Ok(())
        }
        "list" if args.len() == 1 => device::list(),
        "scan" => {
            let o = Options::parse(&args[1..], &[], &["experimental"])?;
            device::scan(o.flag("experimental"))
        }
        "models" if args.len() == 1 => {
            models();
            Ok(())
        }
        "demo" if args.len() == 1 => demo(),
        "protocols" if args.len() == 1 => {
            println!(
                "{}",
                serde_json::to_string_pretty(&unidevice::protocols::summary())
                    .map_err(|_| Error::Invalid("protocol summary serialization".into()))?
            );
            Ok(())
        }
        "razer-probe" => {
            let o = Options::parse(&args[1..], &["device"], &["demo", "experimental"])?;
            let observed = if o.flag("demo") {
                if o.flag("experimental") || o.values.contains_key("device") {
                    return Err(Error::Invalid(
                        "razer demo cannot have hardware flags".into(),
                    ));
                }
                unidevice::razer::demo_observation()?
            } else {
                require_hardware(o.flag("experimental"))?;
                unidevice::razer::probe_native(o.number("device")?)?
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&observed)
                    .map_err(|_| Error::Invalid("Razer observation serialization".into()))?
            );
            Ok(())
        }
        "inspect" => {
            let o = Options::parse(&args[1..], &["device"], &[])?;
            device::inspect(o.number("device")?)
        }
        "dpi" => {
            let o = Options::parse(
                &args[1..],
                &["model", "value", "device"],
                &["apply", "experimental"],
            )?;
            dpi(o.model()?, o.number("value")?, &o)
        }
        "rgb" => {
            let o = Options::parse(&args[1..], &["model", "color", "zone"], &[])?;
            preview_rgb(
                o.model()?,
                parse_color(o.value("color")?)?,
                o.number("zone")?,
            )
        }
        "status" => {
            let o = Options::parse(&args[1..], &["model", "device"], &["experimental"])?;
            require_hardware(o.flag("experimental"))?;
            let mut session =
                Session::connect(NativeTransport::new(device::open(o.model()?, o.index()?)?))?;
            let dpi = session.read_dpi()?;
            let supported = session.supported_dpi()?;
            println!("HID++ 2201 discovered dynamically. Device reports {dpi} DPI.");
            println!("Supported DPI: {supported:?}\nPhysical behavior remains UNVERIFIED.");
            Ok(())
        }
        "profile" => profile(&args[1..]),
        "idle" => {
            let o = Options::parse(&args[1..], &["seconds"], &[])?;
            let seconds: u64 = o.number("seconds")?;
            if !(1..=300).contains(&seconds) {
                return Err(Error::Invalid("idle seconds must be 1..300".into()));
            }
            println!("Offline idle hold: {seconds}s. No device access or polling.");
            std::thread::sleep(std::time::Duration::from_secs(seconds));
            Ok(())
        }
        _ => Err(Error::Invalid(
            "unknown command or unexpected arguments; use --help".into(),
        )),
    }
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
