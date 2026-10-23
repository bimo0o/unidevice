use crate::device;
use crate::error::{Error, Result};
use crate::hidpp::Session;
use crate::transport::NativeTransport;
use std::ffi::CString;
use std::sync::{mpsc, Arc};

#[derive(Clone)]
pub struct DeviceToken {
    path: CString,
    vid: u16,
    pid: u16,
    usage_page: u16,
    usage: u16,
    interface: i32,
}

#[derive(Clone)]
pub struct DeviceRow {
    pub title: String,
    pub detail: String,
    pub eligible: bool,
    pub reason: String,
    pub demo: bool,
    token: Option<DeviceToken>,
}

#[derive(Clone, Debug)]
pub struct Capabilities {
    pub current_dpi: u16,
    pub values: Vec<u16>,
    pub demo: bool,
    pub rgb: [u8; 3],
    pub rgb_control: bool,
    pub dpi_source: String,
}

pub enum Command {
    Scan,
    Probe(DeviceRow),
    SetDpi(u16),
    SetRgb([u8; 3]),
    Effect(Option<crate::effects::Effect>),
    Demo,
    Diagnostic(DeviceRow),
}

pub enum Event {
    Devices(Result<Vec<DeviceRow>>),
    Ready(Result<Capabilities>),
    Applied(Result<Capabilities>),
    Diagnostic(Result<String>),
    Effect(Result<bool>),
    Frame([u8; 3]),
}

fn open_token(token: &DeviceToken) -> Result<NativeTransport> {
    let api = device::enumerate()?;
    let d = api
        .device_list()
        .find(|d| {
            d.path() == token.path.as_c_str()
                && d.vendor_id() == token.vid
                && d.product_id() == token.pid
                && d.usage_page() == token.usage_page
                && d.usage() == token.usage
                && d.interface_number() == token.interface
        })
        .ok_or(Error::NotFound)?;
    if d.vendor_id() != 0x046d
        || d.usage_page() != 0xff00
        || !matches!(d.bus_type(), hidapi::BusType::Usb)
    {
        return Err(Error::Safety(
            "only a matching USB HID++ collection may be opened for control".into(),
        ));
    }
    let handle = d.open_device(&api).map_err(|_| Error::Access)?;
    if !device::read_descriptor(&handle)?.hidpp_framing() {
        return Err(Error::Unsupported(
            "descriptor does not match the documented HID++ framing".into(),
        ));
    }
    Ok(NativeTransport::new(handle))
}

fn scan() -> Result<Vec<DeviceRow>> {
    let api = device::enumerate()?;
    let mut rows = Vec::new();
    for d in api.device_list() {
        if (d.vendor_id(), d.product_id()) == (0x1b1c, 0x1b75)
            && (d.usage_page(), d.usage()) != (0xffc2, 4)
        {
            continue;
        }
        if !(d.usage_page() >= 0xff00
            || d.usage_page() == 1 && matches!(d.usage(), 2 | 6)
            || matches!(d.vendor_id(), 0x046d | 0x1b1c))
        {
            continue;
        }
        let known = device::label(d.vendor_id(), d.product_id());
        let title = if known.starts_with("Unknown") {
            d.product_string()
                .unwrap_or("HID-устройство")
                .chars()
                .filter(|c| !c.is_control())
                .take(72)
                .collect()
        } else {
            known.replace(" (wired)", "")
        };
        let mut reason =
            "Протокол управления пока не определён. Неизвестные команды заблокированы.".to_string();
        let corsair = (d.vendor_id(), d.product_id(), d.usage_page(), d.usage())
            == (0x1b1c, 0x1b75, 0xffc2, 4);
        let eligible = if (d.vendor_id() == 0x046d && d.usage_page() == 0xff00 || corsair)
            && matches!(d.bus_type(), hidapi::BusType::Usb)
        {
            match d
                .open_device(&api)
                .map_err(|_| Error::Access)
                .and_then(|h| device::read_descriptor(&h))
            {
                Ok(descriptor)
                    if (!corsair && descriptor.hidpp_framing())
                        || (corsair
                            && descriptor.reports.iter().any(|r| {
                                r.kind == crate::descriptor::Kind::Feature
                                    && r.id == 0
                                    && r.bytes() == 64
                            })) =>
                {
                    reason = "Можно прочитать настройки устройства.".into();
                    true
                }
                Ok(_) => {
                    reason =
                        "Эта коллекция HID не предназначена для поддерживаемого управления DPI."
                            .into();
                    false
                }
                Err(_) => {
                    reason = "Нет доступа к описанию HID. Закройте фирменное ПО и обновите список."
                        .into();
                    false
                }
            }
        } else {
            false
        };
        if d.vendor_id() == 0x1b1c && !eligible {
            reason =
                "Управление Corsair отключено: ещё не подтверждены безопасные команды этой модели."
                    .into();
        }
        rows.push(DeviceRow {
            title,
            detail: format!(
                "USB · {:04X}:{:04X} · интерфейс {}",
                d.vendor_id(),
                d.product_id(),
                d.interface_number()
            ),
            eligible,
            reason,
            demo: false,
            token: Some(DeviceToken {
                path: d.path().to_owned(),
                vid: d.vendor_id(),
                pid: d.product_id(),
                usage_page: d.usage_page(),
                usage: d.usage(),
                interface: d.interface_number(),
            }),
        });
    }
    rows.sort_by_key(|r| !r.eligible);
    Ok(rows)
}

pub fn demo_rows() -> Vec<DeviceRow> {
    vec![DeviceRow {
        title: "Demo Mouse".into(),
        detail: "Демонстрация · DPI и палитра".into(),
        eligible: true,
        reason: "Все изменения остаются внутри приложения.".into(),
        demo: true,
        token: None,
    }]
}

enum Active {
    Harpoon {
        session: crate::corsair::Session<crate::corsair::NativeFeature>,
        caps: Capabilities,
    },
    Real {
        session: Session<NativeTransport>,
        caps: Capabilities,
    },
    Demo(Capabilities),
}

#[derive(Default)]
struct Backend {
    active: Option<Active>,
    animation: Option<(crate::effects::Effect, std::time::Instant, u8)>,
}
impl Backend {
    fn effect(&mut self, effect: Option<crate::effects::Effect>) -> Result<bool> {
        self.animation = None;
        let Some(effect) = effect else {
            return Ok(false);
        };
        if !effect.valid() {
            return Err(Error::Invalid("effect period must be 4..30 seconds".into()));
        }
        let stage = match self.active.as_mut() {
            Some(Active::Harpoon { session, .. }) => session.state()?.stage,
            Some(Active::Demo(_)) => 0,
            _ => {
                return Err(Error::Unsupported(
                    "RGB animation requires a supported RGB device".into(),
                ))
            }
        };
        self.animation = Some((effect, std::time::Instant::now(), stage));
        Ok(true)
    }

    fn frame(&mut self) -> Result<[u8; 3]> {
        let (effect, start, stage) = self
            .animation
            .ok_or(Error::Safety("animation is stopped".into()))?;
        let result = (|| {
            let color = effect.color(start.elapsed());
            match self.active.as_mut() {
                Some(Active::Harpoon { session, caps }) => {
                    let state = session.set_rgb_expected(color, Some(stage))?;
                    caps.rgb = state.rgb;
                    caps.current_dpi = state.x;
                }
                Some(Active::Demo(caps)) => caps.rgb = color,
                _ => return Err(Error::Safety("RGB session unavailable".into())),
            }
            Ok(color)
        })();
        if result.is_err() {
            self.animation = None;
            self.active = None;
        }
        result
    }

    fn probe(&mut self, row: &DeviceRow) -> Result<Capabilities> {
        self.active = None;
        if row.demo {
            let caps = Capabilities {
                current_dpi: 800,
                values: vec![400, 800, 1200, 1600, 2400, 3200, 6400],
                demo: true,
                rgb: [83, 129, 255],
                rgb_control: true,
                dpi_source: "значения симулятора".into(),
            };
            self.active = Some(Active::Demo(caps.clone()));
            return Ok(caps);
        }
        if !row.eligible {
            return Err(Error::Unsupported(row.reason.clone()));
        }
        let token = row.token.as_ref().ok_or(Error::NotFound)?;
        if (token.vid, token.pid, token.usage_page, token.usage) == (0x1b1c, 0x1b75, 0xffc2, 4) {
            let api = device::enumerate()?;
            let d = api
                .device_list()
                .find(|d| {
                    d.path() == token.path.as_c_str()
                        && d.vendor_id() == token.vid
                        && d.product_id() == token.pid
                        && d.usage_page() == token.usage_page
                        && d.usage() == token.usage
                        && d.interface_number() == token.interface
                        && matches!(d.bus_type(), hidapi::BusType::Usb)
                })
                .ok_or(Error::NotFound)?;
            let mut session =
                crate::corsair::Session::connect(crate::corsair::NativeFeature::new(
                    d.open_device(&api).map_err(|_| Error::Access)?,
                )?)?;
            let state = session.state()?;
            let caps = Capabilities {
                current_dpi: state.x,
                values: (100..=12000).step_by(100).collect(),
                demo: false,
                rgb: state.rgb,
                rgb_control: true,
                dpi_source: "диапазон приложения: 100–12000, шаг 100".into(),
            };
            self.active = Some(Active::Harpoon {
                session,
                caps: caps.clone(),
            });
            return Ok(caps);
        }
        let mut session =
            Session::connect(open_token(row.token.as_ref().ok_or(Error::NotFound)?)?)?;
        let values: Vec<u16> = session
            .supported_dpi()?
            .into_iter()
            .filter(|v| (100..=25600).contains(v))
            .collect();
        if values.is_empty() {
            return Err(Error::Unsupported(
                "no DPI values inside the application policy".into(),
            ));
        }
        let caps = Capabilities {
            current_dpi: session.read_dpi()?,
            values,
            demo: false,
            rgb: [83, 129, 255],
            rgb_control: false,
            dpi_source: "значения получены от устройства".into(),
        };
        self.active = Some(Active::Real {
            session,
            caps: caps.clone(),
        });
        Ok(caps)
    }

    fn apply(&mut self, value: u16) -> Result<Capabilities> {
        let result = (|| {
            let active = self.active.as_mut().ok_or(Error::Safety(
                "refresh and check the device before applying".into(),
            ))?;
            match active {
                Active::Harpoon { session, caps } => {
                    if !caps.values.contains(&value) {
                        return Err(Error::Invalid("DPI outside application policy".into()));
                    }
                    let state = session.set_dpi(value)?;
                    caps.current_dpi = state.x;
                    caps.rgb = state.rgb;
                    Ok(caps.clone())
                }
                Active::Real { session, caps } => {
                    if !caps.values.contains(&value) {
                        return Err(Error::Unsupported(
                            "value is not advertised by the device".into(),
                        ));
                    }
                    caps.current_dpi = session.set_dpi(value)?;
                    Ok(caps.clone())
                }
                Active::Demo(caps) => {
                    if !caps.values.contains(&value) {
                        return Err(Error::Unsupported(
                            "DPI is outside demo capabilities".into(),
                        ));
                    }
                    caps.current_dpi = value;
                    Ok(caps.clone())
                }
            }
        })();
        if result.is_err() {
            self.active = None;
        }
        result
    }

    fn rgb(&mut self, value: [u8; 3]) -> Result<Capabilities> {
        let result = match self.active.as_mut() {
            Some(Active::Harpoon { session, caps }) => session.set_stage_rgb(value).map(|s| {
                caps.current_dpi = s.x;
                caps.rgb = s.rgb;
                caps.clone()
            }),
            Some(Active::Demo(caps)) => {
                caps.rgb = value;
                Ok(caps.clone())
            }
            _ => Err(Error::Safety(
                "no verified RGB hardware transport; no packets sent".into(),
            )),
        };
        if result.is_err() {
            self.active = None;
        }
        result
    }
}

fn diagnostic(row: &DeviceRow) -> Result<String> {
    let mut report = serde_json::json!({"version":1,"app":"UniDevice","app_version":env!("CARGO_PKG_VERSION"),"demo":row.demo,
        "device_name":row.title,"hardware_verified":false,"reports":[],
        "blocked":["firmware","bootloader","reset","flash","onboard profiles","unknown commands"]});
    if !row.demo {
        let token = row.token.as_ref().ok_or(Error::NotFound)?;
        let api = device::enumerate()?;
        let d = api
            .device_list()
            .find(|d| {
                d.path() == token.path.as_c_str()
                    && d.vendor_id() == token.vid
                    && d.product_id() == token.pid
                    && d.usage_page() == token.usage_page
                    && d.usage() == token.usage
                    && d.interface_number() == token.interface
            })
            .ok_or(Error::NotFound)?;
        let handle = d.open_device(&api).map_err(|_| Error::Access)?;
        let descriptor = device::read_descriptor(&handle)?;
        report["vid"] = serde_json::json!(format!("{:04X}", token.vid));
        report["pid"] = serde_json::json!(format!("{:04X}", token.pid));
        report["usage_page"] = serde_json::json!(token.usage_page);
        report["usage"] = serde_json::json!(token.usage);
        report["interface"] = serde_json::json!(token.interface);
        report["descriptor_origin"] = serde_json::json!("hidapi; may be reconstructed on Windows");
        report["reports"] = serde_json::json!(descriptor
            .reports
            .iter()
            .map(|r| serde_json::json!({
            "kind":format!("{:?}",r.kind),"id":r.id,"bytes":r.bytes()}))
            .collect::<Vec<_>>());
    }
    serde_json::to_string_pretty(&report).map_err(|_| Error::Io("diagnostic serialization".into()))
}

fn save_diagnostic(row: &DeviceRow) -> Result<String> {
    use std::io::Write;
    let text = diagnostic(row)?;
    let directory = std::env::current_exe()
        .map_err(|_| Error::Io("application directory".into()))?
        .parent()
        .ok_or(Error::Io("application directory".into()))?
        .to_path_buf();
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("unidevice-diagnostic-{stamp}.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(directory.join(&name))
        .map_err(|_| Error::Io("cannot create diagnostic file".into()))?;
    file.write_all(text.as_bytes())
        .map_err(|_| Error::Io("cannot write diagnostic file".into()))?;
    Ok(name)
}

pub struct Worker {
    pub events: mpsc::Receiver<Event>,
    commands: Option<mpsc::SyncSender<Command>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn start(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(1);
        let (sender, events) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("unidevice-hid".into())
            .spawn(move || {
                let mut backend = Backend::default();
                loop {
                    let command = if backend.animation.is_some() {
                        match receiver.recv_timeout(std::time::Duration::from_millis(250)) {
                            Ok(command) => command,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                let event = match backend.frame() {
                                    Ok(color) => Event::Frame(color),
                                    Err(e) => Event::Effect(Err(e)),
                                };
                                if sender.send(event).is_err() {
                                    break;
                                }
                                wake();
                                continue;
                            }
                        }
                    } else {
                        match receiver.recv() {
                            Ok(command) => command,
                            Err(_) => break,
                        }
                    };
                    if !matches!(command, Command::Effect(_)) {
                        backend.animation = None;
                    }
                    let event = match command {
                        Command::Effect(effect) => Event::Effect(backend.effect(effect)),
                        Command::Scan => {
                            backend.active = None;
                            Event::Devices(scan())
                        }
                        Command::Demo => {
                            backend.active = None;
                            Event::Devices(Ok(demo_rows()))
                        }
                        Command::Probe(row) => Event::Ready(backend.probe(&row)),
                        Command::SetDpi(value) => Event::Applied(backend.apply(value)),
                        Command::SetRgb(value) => Event::Applied(backend.rgb(value)),
                        Command::Diagnostic(row) => Event::Diagnostic(save_diagnostic(&row)),
                    };
                    if sender.send(event).is_err() {
                        break;
                    }
                    wake();
                }
            })
            .map_err(|_| Error::Io("cannot start HID worker".into()))?;
        Ok(Self {
            events,
            commands: Some(commands),
            thread: Some(thread),
        })
    }

    pub fn send(&self, command: Command) -> Result<()> {
        self.commands
            .as_ref()
            .ok_or(Error::Disconnected)?
            .try_send(command)
            .map_err(|_| Error::Safety("another operation is already running".into()))
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        drop(self.commands.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn user_error(error: &Error) -> String {
    match error {
        Error::NotFound | Error::Disconnected => "Устройство отключено или недоступно. Обновите список и выберите его заново.".into(),
        Error::Access => "Нет доступа к устройству. Закройте G HUB/iCUE и обновите список.".into(),
        Error::Timeout => "Устройство не ответило. Повторной записи не было. Обновите список перед новой попыткой.".into(),
        Error::Rejected(_) => "Устройство отклонило команду. Управление приостановлено до новой проверки.".into(),
        Error::Protocol(_) | Error::Safety(_) | Error::ShortWrite => "Проверка безопасности не пройдена. Дальнейшие команды заблокированы; обновите список.".into(),
        Error::Unsupported(_) => "Для этого устройства не обнаружена поддерживаемая настройка. Неизвестные команды не отправляются.".into(),
        _ => "Не удалось выполнить действие. Настройки автоматически не повторяются.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn effects_require_start_stop_cancels_frames_and_fault_ends_animation() {
        let mut b = Backend::default();
        b.probe(&demo_rows()[0]).unwrap();
        assert!(b.frame().is_err());
        let effect = crate::effects::Effect {
            pattern: crate::effects::Pattern::Gradient,
            first: [255, 0, 0],
            second: [0, 0, 255],
            seconds: 4.0,
        };
        assert!(b.effect(Some(effect)).unwrap());
        assert!(b.frame().is_ok());
        assert!(!b.effect(None).unwrap());
        assert!(b.frame().is_err());
        assert_eq!(b.apply(1600).unwrap().current_dpi, 1600);
        b.effect(Some(effect)).unwrap();
        b.active = None;
        assert!(b.frame().is_err());
        assert!(b.animation.is_none());
        assert!(b.active.is_none());
    }
    #[test]
    fn worker_stop_prevents_more_frames() {
        use std::time::Duration;
        let w = Worker::start(Arc::new(|| {})).unwrap();
        w.send(Command::Probe(demo_rows().remove(0))).unwrap();
        assert!(matches!(
            w.events.recv_timeout(Duration::from_secs(1)).unwrap(),
            Event::Ready(Ok(_))
        ));
        w.send(Command::Effect(Some(crate::effects::Effect {
            pattern: crate::effects::Pattern::Rainbow,
            first: [0; 3],
            second: [0; 3],
            seconds: 6.0,
        })))
        .unwrap();
        assert!(matches!(
            w.events.recv_timeout(Duration::from_secs(1)).unwrap(),
            Event::Effect(Ok(true))
        ));
        assert!(matches!(
            w.events.recv_timeout(Duration::from_secs(1)).unwrap(),
            Event::Frame(_)
        ));
        w.send(Command::Effect(None)).unwrap();
        loop {
            if matches!(
                w.events.recv_timeout(Duration::from_secs(1)).unwrap(),
                Event::Effect(Ok(false))
            ) {
                break;
            }
        }
        assert!(matches!(
            w.events.recv_timeout(Duration::from_millis(600)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
    }
    #[test]
    fn no_apply_without_probe_and_no_unknown_rgb_transport() {
        let mut backend = Backend::default();
        assert!(backend.apply(800).is_err());
        assert!(backend.rgb([255, 0, 0]).is_err());
        assert!(backend.active.is_none());
    }
    #[test]
    fn demo_has_no_device_token_and_cannot_open_real_transport() {
        let row = demo_rows().remove(0);
        assert!(row.token.is_none());
        let mut backend = Backend::default();
        assert_eq!(backend.probe(&row).unwrap().current_dpi, 800);
        assert_eq!(backend.apply(1600).unwrap().current_dpi, 1600);
        assert_eq!(backend.rgb([255, 0, 0]).unwrap().rgb, [255, 0, 0]);
        assert!(matches!(backend.active, Some(Active::Demo(_))));
        assert!(backend.apply(801).is_err());
        assert!(backend.active.is_none());
    }
    #[test]
    fn unsupported_selection_drops_previous_session() {
        let mut backend = Backend::default();
        let mut row = demo_rows().remove(0);
        backend.probe(&row).unwrap();
        row.demo = false;
        row.eligible = false;
        assert!(backend.probe(&row).is_err());
        assert!(backend.active.is_none());
    }

    #[test]
    fn diagnostic_excludes_unique_ids_and_never_reads_hardware_in_demo() {
        let text = diagnostic(&demo_rows()[0]).unwrap();
        for forbidden in ["serial", "device_path", "unique_id"] {
            assert!(!text.contains(forbidden));
        }
        assert!(text.contains("\"hardware_verified\": false"));
    }
}
