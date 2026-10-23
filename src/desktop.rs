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
    pub package: Option<crate::compatibility::Pack>,
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
impl DeviceRow {
    pub fn can_probe_razer(&self) -> bool {
        self.token.as_ref().is_some_and(|t| {
            t.vid == 0x1532
                && crate::razer::profile(t.pid).is_some_and(|p| t.interface == p.usb_interface)
        })
    }
    pub fn identity(&self) -> Option<(u16, u16)> {
        self.token.as_ref().map(|t| (t.vid, t.pid))
    }
    fn same_device(&self, other: &Self) -> bool {
        match (&self.token, &other.token) {
            (Some(a), Some(b)) => a.path == b.path,
            (None, None) => self.title == other.title,
            _ => false,
        }
    }
}
impl Capabilities {
    pub fn passport(&self) -> crate::passport::Passport {
        crate::passport::Passport::for_caps(self.demo, self.rgb_control, &self.dpi_source)
    }
    fn narrow(&mut self, row: &DeviceRow) {
        if let Some(max) = row.package.as_ref().and_then(|p| p.max_dpi) {
            self.values.retain(|v| *v <= max);
        }
    }
}

pub enum Command {
    Scan,
    Probe(DeviceRow),
    SetDpi(u16),
    SetRgb([u8; 3]),
    Effect(Option<crate::effects::Effect>),
    Demo,
    Diagnostic(DeviceRow),
    SceneStart(Vec<SceneSelection>, crate::effects::Effect),
    SceneStop,
    RestoreCheckpoint,
    ReadOnlyRazer(DeviceRow),
    Transfer {
        dpi: Option<u16>,
        rgb: Option<[u8; 3]>,
    },
}

pub enum Event {
    Devices(Result<Vec<DeviceRow>>),
    Ready(Result<Capabilities>),
    Applied(Result<Capabilities>),
    Diagnostic(Result<String>),
    Effect(Result<bool>),
    Frame([u8; 3]),
    Scene(Result<Vec<SceneStatus>>),
    SceneFrame(Vec<SceneStatus>),
    Transfer(Result<(Capabilities, Vec<String>)>),
    Recovery(Option<crate::recovery::RecoveryInfo>),
    ReadOnlyRazer(Result<(crate::razer::Observation, String)>),
}

#[derive(Clone)]
pub struct SceneSelection {
    pub row: DeviceRow,
    pub brightness: f32,
    pub phase_ms: i32,
}
#[derive(Clone, Debug)]
pub struct SceneStatus {
    pub name: String,
    pub color: [u8; 3],
    pub dpi: u16,
    pub frames: u64,
}
struct SceneChannel {
    epoch: std::time::Instant,
    status: Arc<std::sync::Mutex<SceneStatus>>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    handle: Option<std::thread::JoinHandle<()>>,
}
impl Drop for SceneChannel {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(handle) = self.handle.take() {
            handle.thread().unpark();
            let _ = handle.join();
        }
    }
}
struct SceneController {
    members: Vec<SceneChannel>,
    stop: Arc<std::sync::atomic::AtomicBool>,
    failure: Arc<std::sync::Mutex<Option<Error>>>,
}
impl Drop for SceneController {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        for channel in &self.members {
            if let Some(handle) = &channel.handle {
                handle.thread().unpark();
            }
        }
    }
}
impl SceneController {
    fn start(selections: Vec<SceneSelection>, effect: crate::effects::Effect) -> Result<Self> {
        if selections.is_empty() || selections.len() > 8 || !effect.valid() {
            return Err(Error::Invalid(
                "select 1..8 RGB targets and a valid effect".into(),
            ));
        }
        let mut ready: Vec<(SceneSelection, Backend)> = Vec::new();
        for s in selections {
            if !s.brightness.is_finite()
                || !(0.0..=1.0).contains(&s.brightness)
                || !(-500..=500).contains(&s.phase_ms)
                || ready.iter().any(|(other, _)| s.row.same_device(&other.row))
            {
                return Err(Error::Invalid(
                    "duplicate target or invalid scene compensation".into(),
                ));
            }
            let mut backend = Backend::default();
            let caps = backend.probe(&s.row)?;
            if !caps.rgb_control {
                return Err(Error::Unsupported(format!(
                    "{} has no verified RGB adapter",
                    s.row.title
                )));
            }
            backend.effect(Some(effect.clone()))?;
            ready.push((s, backend));
        }
        let epoch = std::time::Instant::now() + std::time::Duration::from_millis(20);
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let failure = Arc::new(std::sync::Mutex::new(None));
        let mut members = Vec::new();
        for (selection, mut backend) in ready {
            let caps = backend.caps().unwrap();
            let status = Arc::new(std::sync::Mutex::new(SceneStatus {
                name: selection.row.title.clone(),
                color: caps.rgb,
                dpi: caps.current_dpi,
                frames: 0,
            }));
            backend.animation.as_mut().unwrap().1 = epoch;
            let thread_status = status.clone();
            let thread_stop = stop.clone();
            let thread_failure = failure.clone();
            let thread_effect = effect.clone();
            let handle = std::thread::Builder::new()
                .name("unidevice-scene-device".into())
                .spawn(move || {
                    let mut next_frame = epoch;
                    while !thread_stop.load(std::sync::atomic::Ordering::Acquire) {
                        let started = std::time::Instant::now();
                        if started < next_frame {
                            std::thread::park_timeout(next_frame - started);
                            continue;
                        }
                        let seconds = (started.saturating_duration_since(epoch).as_secs_f64()
                            + f64::from(selection.phase_ms) / 1000.0)
                            .rem_euclid(f64::from(thread_effect.seconds));
                        let color = thread_effect
                            .color(std::time::Duration::from_secs_f64(seconds))
                            .map(|v| (f32::from(v) * selection.brightness).round() as u8);
                        match backend.render_frame(color) {
                            Ok(color) => {
                                if let Ok(mut state) = thread_status.lock() {
                                    state.color = color;
                                    state.dpi = backend.caps().unwrap().current_dpi;
                                    state.frames += 1;
                                }
                            }
                            Err(error) => {
                                if let Ok(mut state) = thread_failure.lock() {
                                    if state.is_none() {
                                        *state = Some(error);
                                    }
                                }
                                thread_stop.store(true, std::sync::atomic::Ordering::Release);
                                break;
                            }
                        }
                        let wait = std::time::Duration::from_millis(16)
                            .saturating_sub(started.elapsed())
                            .max(std::time::Duration::from_millis(4));
                        if thread_stop.load(std::sync::atomic::Ordering::Acquire) {
                            break;
                        }
                        next_frame = std::time::Instant::now() + wait;
                    }
                })
                .map_err(|_| Error::Io("cannot start scene device channel".into()))?;
            members.push(SceneChannel {
                epoch,
                status,
                stop: stop.clone(),
                handle: Some(handle),
            });
        }
        Ok(Self {
            members,
            stop,
            failure,
        })
    }
    fn statuses(&self) -> Vec<SceneStatus> {
        self.members
            .iter()
            .filter_map(|c| c.status.lock().ok().map(|s| s.clone()))
            .collect()
    }
    fn frame(&mut self) -> Result<Vec<SceneStatus>> {
        if self.members.windows(2).any(|w| w[0].epoch != w[1].epoch)
            || self
                .members
                .iter()
                .any(|m| m.handle.as_ref().is_some_and(|h| h.is_finished()))
                && !self.stop.load(std::sync::atomic::Ordering::Acquire)
        {
            return Err(Error::Safety(
                "scene channel clock or lifecycle mismatch".into(),
            ));
        }
        if self.stop.load(std::sync::atomic::Ordering::Acquire) {
            return Err(self
                .failure
                .lock()
                .ok()
                .and_then(|f| f.clone())
                .unwrap_or(Error::Safety("scene channel stopped".into())));
        }
        Ok(self.statuses())
    }
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
    NativeTransport::new(handle)
}

fn scan() -> Result<Vec<DeviceRow>> {
    let api = device::enumerate()?;
    let packages = crate::compatibility::installed();
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
        let package = packages
            .iter()
            .filter(|p| p.matches(d.vendor_id(), d.product_id()))
            .max_by_key(|p| p.revision)
            .cloned();
        let title = if let Some(pack) = &package {
            pack.name.clone()
        } else if known.starts_with("Unknown") {
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
        if !eligible {
            if let Some(hint) = crate::protocols::hint(d.vendor_id(), d.product_id()) {
                reason = hint;
            }
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
            package,
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
    let first = DeviceRow {
        title: "Demo Mouse".into(),
        detail: "Демонстрация · DPI и палитра".into(),
        eligible: true,
        reason: "Все изменения остаются внутри приложения.".into(),
        demo: true,
        package: None,
        token: None,
    };
    let mut second = first.clone();
    second.title = "Demo Mouse 2".into();
    second.detail = "Второе независимое RGB-устройство".into();
    vec![first, second]
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
    checkpoint: Option<Checkpoint>,
}
struct Checkpoint {
    caps: Capabilities,
    harpoon: Option<crate::corsair::State>,
    file: Option<String>,
}
impl Backend {
    fn recovery_info(&self) -> Option<crate::recovery::RecoveryInfo> {
        let checkpoint = self.checkpoint.as_ref()?;
        Some(crate::recovery::RecoveryInfo {
            dpi: checkpoint.caps.current_dpi,
            rgb: checkpoint.caps.rgb_control.then_some(checkpoint.caps.rgb),
            file: checkpoint.file.clone(),
            can_restore: self.active.is_some(),
        })
    }
    fn ensure_checkpoint(&mut self) -> Result<()> {
        let result = self.capture_checkpoint();
        if result.is_err() {
            self.active = None;
            self.animation = None;
        }
        result
    }
    fn capture_checkpoint(&mut self) -> Result<()> {
        if self.checkpoint.is_some() {
            return Ok(());
        }
        let (caps, harpoon, portable_dpi) = match self.active.as_mut().ok_or(Error::NotFound)? {
            Active::Harpoon { session, caps } => {
                let state = session.state()?;
                caps.current_dpi = state.x;
                caps.rgb = state.rgb;
                let dpi = (state.x == state.y && state.linked_axes()).then_some(state.x);
                (caps.clone(), Some(state), dpi)
            }
            Active::Real { session, caps } => {
                caps.current_dpi = session.read_dpi()?;
                (caps.clone(), None, Some(caps.current_dpi))
            }
            Active::Demo(caps) => (caps.clone(), None, Some(caps.current_dpi)),
        };
        let file = if caps.demo {
            None
        } else {
            Some(crate::recovery::save_settings(
                portable_dpi,
                caps.rgb_control.then_some(caps.rgb),
            )?)
        };
        self.checkpoint = Some(Checkpoint {
            caps,
            harpoon,
            file,
        });
        Ok(())
    }
    fn restore_checkpoint(&mut self) -> Result<Capabilities> {
        self.animation = None;
        let result = (|| {
            let checkpoint = self.checkpoint.as_ref().ok_or(Error::NotFound)?;
            match self
                .active
                .as_mut()
                .ok_or(Error::Safety("session must still be active".into()))?
            {
                Active::Harpoon { session, caps } => {
                    let state = checkpoint
                        .harpoon
                        .as_ref()
                        .ok_or(Error::Safety("adapter changed".into()))?;
                    if !caps.values.contains(&state.x) || !caps.values.contains(&state.y) {
                        return Err(Error::Safety("checkpoint no longer inside policy".into()));
                    }
                    let restored = session.restore_live(state)?;
                    caps.current_dpi = restored.x;
                    caps.rgb = restored.rgb;
                    Ok(caps.clone())
                }
                Active::Real { session, caps } => {
                    if checkpoint.harpoon.is_some()
                        || !caps.values.contains(&checkpoint.caps.current_dpi)
                    {
                        return Err(Error::Safety("checkpoint no longer inside policy".into()));
                    }
                    caps.current_dpi = session.set_dpi(checkpoint.caps.current_dpi)?;
                    Ok(caps.clone())
                }
                Active::Demo(caps) => {
                    *caps = checkpoint.caps.clone();
                    Ok(caps.clone())
                }
            }
        })();
        if result.is_err() {
            self.active = None;
        }
        result
    }
    fn caps(&self) -> Option<&Capabilities> {
        match self.active.as_ref()? {
            Active::Harpoon { caps, .. } | Active::Real { caps, .. } | Active::Demo(caps) => {
                Some(caps)
            }
        }
    }
    fn effect(&mut self, effect: Option<crate::effects::Effect>) -> Result<bool> {
        self.animation = None;
        let Some(effect) = effect else {
            return Ok(false);
        };
        if !effect.valid() {
            return Err(Error::Invalid("effect period must be 4..30 seconds".into()));
        }
        self.ensure_checkpoint()?;
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
            .clone()
            .ok_or(Error::Safety("animation is stopped".into()))?;
        let _ = stage;
        self.render_frame(effect.color(start.elapsed()))
    }
    fn render_frame(&mut self, color: [u8; 3]) -> Result<[u8; 3]> {
        let stage = self
            .animation
            .as_ref()
            .ok_or(Error::Safety("animation is stopped".into()))?
            .2;
        let result = (|| {
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
        self.checkpoint = None;
        self.animation = None;
        if row.demo {
            let mut caps = Capabilities {
                current_dpi: 800,
                values: vec![400, 800, 1000, 1200, 1600, 2000, 2400, 3200, 6400],
                demo: true,
                rgb: [83, 129, 255],
                rgb_control: true,
                dpi_source: "значения симулятора".into(),
            };
            caps.narrow(row);
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
            let mut caps = Capabilities {
                current_dpi: state.x,
                values: (100..=12000).step_by(100).collect(),
                demo: false,
                rgb: state.rgb,
                rgb_control: true,
                dpi_source: "диапазон приложения: 100–12000, шаг 100".into(),
            };
            caps.narrow(row);
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
        let mut caps = Capabilities {
            current_dpi: session.read_dpi()?,
            values,
            demo: false,
            rgb: [83, 129, 255],
            rgb_control: false,
            dpi_source: format!(
                "HID++ 0x{:04X}: значения получены от устройства",
                session.protocol_id()
            ),
        };
        caps.narrow(row);
        if caps.values.is_empty() {
            return Err(Error::Unsupported(
                "package excluded every advertised DPI".into(),
            ));
        }
        self.active = Some(Active::Real {
            session,
            caps: caps.clone(),
        });
        Ok(caps)
    }

    fn apply(&mut self, value: u16) -> Result<Capabilities> {
        let result = (|| {
            self.ensure_checkpoint()?;
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
        self.ensure_checkpoint()?;
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

fn read_only_razer(row: &DeviceRow) -> Result<(crate::razer::Observation, String)> {
    let token = row.token.as_ref().ok_or(Error::NotFound)?;
    let api = device::enumerate()?;
    let info = api
        .device_list()
        .find(|d| {
            d.path() == token.path.as_c_str()
                && d.vendor_id() == token.vid
                && d.product_id() == token.pid
                && d.interface_number() == token.interface
                && d.usage_page() == token.usage_page
                && d.usage() == token.usage
        })
        .ok_or(Error::NotFound)?;
    let observed = crate::razer::probe_info(&api, info)?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("razer-{stamp}");
    let path = crate::storage::path("diagnostics", &name)?;
    let text = serde_json::to_string_pretty(&observed)
        .map_err(|_| Error::Io("Razer report serialization".into()))?;
    crate::storage::save_new(&path, &text)?;
    Ok((observed, format!("diagnostics/{name}.json")))
}

pub struct Worker {
    pub events: mpsc::Receiver<Event>,
    commands: Option<mpsc::SyncSender<Command>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Worker {
    pub fn start(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        Self::start_with_status(wake, false)
    }
    pub fn start_with_recovery(wake: Arc<dyn Fn() + Send + Sync>) -> Result<Self> {
        Self::start_with_status(wake, true)
    }
    fn start_with_status(wake: Arc<dyn Fn() + Send + Sync>, recovery_events: bool) -> Result<Self> {
        let (commands, receiver) = mpsc::sync_channel(1);
        let (sender, events) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("unidevice-hid".into())
            .spawn(move || {
                let mut backend = Backend::default();
                let mut scene: Option<SceneController> = None;
                let mut frame_wait = std::time::Duration::from_millis(16);
                loop {
                    let command = if backend.animation.is_some() || scene.is_some() {
                        match receiver.recv_timeout(frame_wait) {
                            Ok(command) => command,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {
                                let started = std::time::Instant::now();
                                let event = if let Some(controller) = scene.as_mut() {
                                    match controller.frame() {
                                        Ok(status) => Event::SceneFrame(status),
                                        Err(e) => {
                                            scene = None;
                                            Event::Scene(Err(e))
                                        }
                                    }
                                } else {
                                    match backend.frame() {
                                        Ok(color) => Event::Frame(color),
                                        Err(e) => Event::Effect(Err(e)),
                                    }
                                };
                                frame_wait = std::time::Duration::from_millis(if scene.is_some() {
                                    33
                                } else {
                                    16
                                })
                                .saturating_sub(started.elapsed())
                                .max(std::time::Duration::from_millis(4));
                                if recovery_events
                                    && matches!(
                                        &event,
                                        Event::Effect(Err(_)) | Event::Scene(Err(_))
                                    )
                                    && sender
                                        .send(Event::Recovery(backend.recovery_info()))
                                        .is_err()
                                {
                                    break;
                                }
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
                    scene = None;
                    let event = match command {
                        Command::SceneStop => Event::Scene(Ok(Vec::new())),
                        Command::Transfer { dpi, rgb } => Event::Transfer((|| {
                            if dpi.is_none() && rgb.is_none() {
                                return Err(Error::Invalid("no compatible fields to apply".into()));
                            }
                            let mut applied = Vec::new();
                            if let Some(dpi) = dpi {
                                backend.apply(dpi)?;
                                applied.push(format!("DPI {dpi}"));
                            }
                            if let Some(rgb) = rgb {
                                backend.rgb(rgb)?;
                                applied.push("RGB".into());
                            }
                            Ok((backend.caps().cloned().ok_or(Error::NotFound)?, applied))
                        })(
                        )),
                        Command::SceneStart(selections, effect) => {
                            backend.active = None;
                            match SceneController::start(selections, effect) {
                                Ok(controller) => {
                                    let status = controller.statuses();
                                    scene = Some(controller);
                                    Event::Scene(Ok(status))
                                }
                                Err(e) => Event::Scene(Err(e)),
                            }
                        }
                        Command::Effect(effect) => Event::Effect(backend.effect(effect)),
                        Command::Scan => {
                            backend.active = None;
                            backend.checkpoint = None;
                            Event::Devices(scan())
                        }
                        Command::Demo => {
                            backend.active = None;
                            backend.checkpoint = None;
                            Event::Devices(Ok(demo_rows()))
                        }
                        Command::Probe(row) => Event::Ready(backend.probe(&row)),
                        Command::SetDpi(value) => Event::Applied(backend.apply(value)),
                        Command::SetRgb(value) => Event::Applied(backend.rgb(value)),
                        Command::RestoreCheckpoint => Event::Applied(backend.restore_checkpoint()),
                        Command::ReadOnlyRazer(row) => {
                            backend.active = None;
                            backend.checkpoint = None;
                            Event::ReadOnlyRazer(read_only_razer(&row))
                        }
                        Command::Diagnostic(row) => Event::Diagnostic(save_diagnostic(&row)),
                    };
                    if recovery_events
                        && sender
                            .send(Event::Recovery(backend.recovery_info()))
                            .is_err()
                    {
                        break;
                    }
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
    fn checkpoint_retains_baseline_and_cannot_cross_probe_or_fault() {
        let rows = demo_rows();
        let mut backend = Backend::default();
        let original = backend.probe(&rows[0]).unwrap();
        assert!(backend.recovery_info().is_none());
        backend.apply(1600).unwrap();
        backend.rgb([255, 0, 0]).unwrap();
        let saved = backend.recovery_info().unwrap();
        assert_eq!(saved.dpi, original.current_dpi);
        assert_eq!(saved.rgb, Some(original.rgb));
        assert!(saved.file.is_none());
        let restored = backend.restore_checkpoint().unwrap();
        assert_eq!(restored.current_dpi, original.current_dpi);
        assert_eq!(restored.rgb, original.rgb);
        backend.probe(&rows[1]).unwrap();
        assert!(backend.recovery_info().is_none());
        assert!(backend.restore_checkpoint().is_err());
        backend.probe(&rows[0]).unwrap();
        backend.apply(1600).unwrap();
        assert!(backend.apply(801).is_err());
        assert!(!backend.recovery_info().unwrap().can_restore);
        assert!(backend.restore_checkpoint().is_err());
    }
    #[test]
    fn scene_uses_one_clock_and_rejects_duplicate_targets_before_writes() {
        let rows = demo_rows();
        let selections: Vec<_> = rows
            .iter()
            .cloned()
            .map(|row| SceneSelection {
                row,
                brightness: 1.0,
                phase_ms: 0,
            })
            .collect();
        let mut scene =
            SceneController::start(selections, crate::effects::Effect::default()).unwrap();
        assert_eq!(scene.members[0].epoch, scene.members[1].epoch);
        let statuses = scene.frame().unwrap();
        assert_eq!(statuses.len(), 2);
        assert_eq!(statuses[0].dpi, 800);
        assert_eq!(statuses[0].color, statuses[1].color);
        let duplicate = SceneSelection {
            row: rows[0].clone(),
            brightness: 1.0,
            phase_ms: 0,
        };
        assert!(SceneController::start(
            vec![duplicate.clone(), duplicate],
            crate::effects::Effect::default()
        )
        .is_err());
        *scene.failure.lock().unwrap() = Some(Error::Disconnected);
        scene.stop.store(true, std::sync::atomic::Ordering::Release);
        assert!(scene.frame().is_err());
    }
    #[test]
    fn imported_pack_can_only_narrow_runtime_values() {
        let mut row = demo_rows().remove(0);
        row.package = Some(crate::compatibility::Pack {
            version: 1,
            revision: 1,
            id: "limit".into(),
            name: "Limited mouse".into(),
            adapter: crate::compatibility::Adapter::Hidpp2,
            vid: 0x046d,
            pids: vec![1],
            max_dpi: Some(1200),
            evidence: "local".into(),
            sources: vec![],
        });
        let mut backend = Backend::default();
        let caps = backend.probe(&row).unwrap();
        assert_eq!(caps.values, vec![400, 800, 1000, 1200]);
        assert!(backend.apply(1600).is_err());
    }
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
            ..Default::default()
        };
        assert!(b.effect(Some(effect.clone())).unwrap());
        assert!(b.frame().is_ok());
        assert!(!b.effect(None).unwrap());
        assert!(b.frame().is_err());
        assert_eq!(b.apply(1600).unwrap().current_dpi, 1600);
        b.effect(Some(effect.clone())).unwrap();
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
            ..Default::default()
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
    fn shared_scene_stop_prevents_frames_for_every_member() {
        use std::time::Duration;
        let w = Worker::start(Arc::new(|| {})).unwrap();
        let targets = demo_rows()
            .into_iter()
            .map(|row| SceneSelection {
                row,
                brightness: 1.0,
                phase_ms: 0,
            })
            .collect();
        w.send(Command::SceneStart(
            targets,
            crate::effects::Effect::default(),
        ))
        .unwrap();
        assert!(
            matches!(w.events.recv_timeout(Duration::from_secs(1)).unwrap(),Event::Scene(Ok(status)) if status.len()==2)
        );
        assert!(
            matches!(w.events.recv_timeout(Duration::from_secs(1)).unwrap(),Event::SceneFrame(status) if status.len()==2)
        );
        w.send(Command::SceneStop).unwrap();
        loop {
            if matches!(w.events.recv_timeout(Duration::from_secs(1)).unwrap(),Event::Scene(Ok(status)) if status.is_empty())
            {
                break;
            }
        }
        assert!(w.events.recv_timeout(Duration::from_millis(100)).is_err());
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
        assert!(!row.can_probe_razer());
        assert!(read_only_razer(&row).is_err());
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
