use crate::desktop::{self, Capabilities, Command, DeviceRow, Event, Worker};
use crate::device::Model;
use crate::profile::Profile;
use eframe::egui::{self, Color32, RichText, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::Arc;

const ACCENT: Color32 = Color32::from_rgb(74, 105, 235);
const INK: Color32 = Color32::from_rgb(30, 39, 60);
const MUTED: Color32 = Color32::from_rgb(110, 122, 145);
const BG: Color32 = Color32::from_rgb(245, 247, 252);
const LINE: Color32 = Color32::from_rgb(229, 234, 243);

pub fn run(demo: bool, capture: Option<PathBuf>) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 760.0])
            .with_min_inner_size([900.0, 660.0])
            .with_title("UniDevice"),
        renderer: eframe::Renderer::Glow,
        persist_window: false,
        ..Default::default()
    };
    eframe::run_native(
        "UniDevice",
        options,
        Box::new(move |cc| Ok(Box::new(DesktopApp::new(&cc.egui_ctx, demo, capture)?))),
    )
}

enum Action {
    Scan,
    Demo,
    Probe(usize),
    Dpi,
    Rgb,
    Save,
    Load,
    Diagnostic,
}

pub struct DesktopApp {
    worker: Worker,
    rows: Vec<DeviceRow>,
    selected: Option<usize>,
    caps: Option<Capabilities>,
    requested: u16,
    color: [u8; 3],
    busy: bool,
    demo: bool,
    notice: String,
    failed: bool,
    profile_name: String,
    capture: Option<PathBuf>,
    capture_requested: bool,
}

impl DesktopApp {
    fn new(
        ctx: &egui::Context,
        demo: bool,
        capture: Option<PathBuf>,
    ) -> crate::error::Result<Self> {
        let mut style = (*ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = BG;
        style.visuals.override_text_color = Some(INK);
        style.visuals.selection.bg_fill = ACCENT;
        style.visuals.widgets.inactive.corner_radius = 8.into();
        style.visuals.widgets.hovered.corner_radius = 8.into();
        style.visuals.widgets.active.corner_radius = 8.into();
        style.spacing.item_spacing = Vec2::new(10.0, 10.0);
        style.spacing.button_padding = Vec2::new(14.0, 9.0);
        for (kind, size) in [
            (egui::TextStyle::Body, 15.0),
            (egui::TextStyle::Small, 12.0),
            (egui::TextStyle::Button, 14.0),
            (egui::TextStyle::Heading, 25.0),
        ] {
            style
                .text_styles
                .insert(kind, egui::FontId::proportional(size));
        }
        ctx.set_style(style);
        let wake_ctx = ctx.clone();
        let worker = Worker::start(Arc::new(move || wake_ctx.request_repaint()))?;
        worker.send(if demo { Command::Demo } else { Command::Scan })?;
        Ok(Self {
            worker,
            rows: Vec::new(),
            selected: None,
            caps: None,
            requested: 800,
            color: [83, 129, 255],
            busy: true,
            demo,
            notice: "Ищем подключённые устройства…".into(),
            failed: false,
            profile_name: "my-profile".into(),
            capture,
            capture_requested: false,
        })
    }

    fn send(&mut self, command: Command) {
        if self.busy {
            return;
        }
        match self.worker.send(command) {
            Ok(()) => {
                self.busy = true;
                self.failed = false;
                self.notice = "Проверяем ответ устройства…".into();
            }
            Err(e) => {
                self.failed = true;
                self.notice = desktop::user_error(&e);
            }
        }
    }

    fn poll(&mut self) {
        while let Ok(event) = self.worker.events.try_recv() {
            self.busy = false;
            match event {
                Event::Devices(result) => {
                    self.caps = None;
                    self.selected = None;
                    match result {
                        Ok(rows) => {
                            self.rows = rows;
                            self.notice = if self.rows.is_empty() {
                                "Подключите мышь и нажмите «Обновить».".into()
                            } else {
                                "Выберите устройство слева. Настройки при обнаружении не меняются."
                                    .into()
                            };
                            let candidates: Vec<_> = self
                                .rows
                                .iter()
                                .enumerate()
                                .filter(|(_, r)| r.eligible)
                                .map(|(i, _)| i)
                                .collect();
                            if candidates.len() == 1 {
                                self.action(Action::Probe(candidates[0]));
                            } else if self.rows.len() == 1 {
                                self.selected = Some(0);
                            }
                        }
                        Err(e) => {
                            self.rows.clear();
                            self.failed = true;
                            self.notice = desktop::user_error(&e);
                        }
                    }
                }
                Event::Ready(result) | Event::Applied(result) => match result {
                    Ok(caps) => {
                        self.requested = caps.current_dpi;
                        self.color = caps.rgb;
                        self.notice = if caps.demo {
                            "Демонстрация: физическое устройство не изменяется.".into()
                        } else {
                            format!("Устройство подтвердило {} DPI. Команды сохранения в аппаратную память не отправлялись.",caps.current_dpi)
                        };
                        self.caps = Some(caps);
                        self.failed = false;
                    }
                    Err(e) => {
                        self.caps = None;
                        self.failed = true;
                        self.notice = desktop::user_error(&e);
                    }
                },
                Event::Diagnostic(result) => match result {
                    Ok(name) => {
                        self.failed = false;
                        self.notice=format!("Диагностика сохранена рядом с приложением: {name}. Серийных номеров и ввода с клавиатуры в ней нет.");
                    }
                    Err(_) => {
                        self.failed = true;
                        self.notice="Не удалось сохранить диагностику. Проверьте подключение и права на папку приложения.".into();
                    }
                },
            }
        }
    }

    fn action(&mut self, action: Action) {
        if self.busy {
            return;
        }
        match action {
            Action::Scan => {
                self.demo = false;
                self.caps = None;
                self.selected = None;
                self.send(Command::Scan);
            }
            Action::Demo => {
                self.demo = true;
                self.caps = None;
                self.selected = None;
                self.send(Command::Demo);
            }
            Action::Probe(index) => {
                if let Some(row) = self.rows.get(index).cloned() {
                    self.selected = Some(index);
                    self.caps = None;
                    self.failed = false;
                    if row.eligible {
                        self.send(Command::Probe(row));
                    } else {
                        self.notice = row.reason.clone();
                        self.send(Command::Probe(row));
                    }
                }
            }
            Action::Dpi => {
                self.send(Command::SetDpi(self.requested));
            }
            Action::Rgb => {
                self.send(Command::SetDemoRgb(self.color));
            }
            Action::Diagnostic => {
                if let Some(row) = self.selected.and_then(|i| self.rows.get(i)).cloned() {
                    self.send(Command::Diagnostic(row));
                }
            }
            Action::Save | Action::Load => {
                let result = (|| -> crate::error::Result<String> {
                    let path = profile_path(&self.profile_name)?;
                    if matches!(action, Action::Save) {
                        if self.caps.is_none() {
                            return Err(crate::error::Error::Invalid("no capabilities".into()));
                        }
                        std::fs::create_dir_all(path.parent().unwrap()).map_err(|_| {
                            crate::error::Error::Io("cannot create profiles folder".into())
                        })?;
                        Profile {
                            version: 1,
                            model: Model::Auto,
                            dpi: Some(self.requested),
                            rgb: None,
                            zone: None,
                        }
                        .save(&path)?;
                        Ok("Профиль DPI сохранён в папке profiles рядом с приложением.".into())
                    } else {
                        let profile = Profile::load(&path)?;
                        let value = profile
                            .dpi
                            .ok_or(crate::error::Error::Unsupported("not a DPI profile".into()))?;
                        if !self
                            .caps
                            .as_ref()
                            .is_some_and(|c| c.values.contains(&value))
                        {
                            return Err(crate::error::Error::Unsupported(
                                "DPI not advertised".into(),
                            ));
                        }
                        self.requested = value;
                        Ok("Профиль загружен в интерфейс. Устройство изменится только по кнопке «Применить».".into())
                    }
                })();
                match result {
                    Ok(text) => {
                        self.notice = text;
                        self.failed = false;
                    }
                    Err(_) => {
                        self.notice="Не удалось открыть или сохранить профиль. Проверьте имя и права; существующие файлы не перезаписываются.".into();
                        self.failed = true;
                    }
                }
            }
        }
    }

    fn ui(&mut self, ctx: &egui::Context) {
        let mut action = None;
        egui::SidePanel::left("devices")
            .exact_width(264.0)
            .frame(egui::Frame::new().fill(Color32::WHITE).inner_margin(20))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(30.0, 30.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 9, ACCENT);
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "U",
                        egui::FontId::proportional(22.0),
                        Color32::WHITE,
                    );
                    ui.label(RichText::new("UniDevice").size(23.0).strong());
                });
                ui.add_space(26.0);
                ui.label(RichText::new("УСТРОЙСТВА").size(11.0).color(MUTED).strong());
                ui.add_space(8.0);
                egui::ScrollArea::vertical()
                    .max_height(ui.available_height() - 155.0)
                    .show(ui, |ui| {
                        if self.rows.is_empty() {
                            ui.label(RichText::new("Пока ничего не найдено").color(MUTED));
                        }
                        for (index, row) in self.rows.iter().enumerate() {
                            let selected = self.selected == Some(index);
                            let response = egui::Frame::new()
                                .fill(if selected {
                                    Color32::from_rgb(237, 241, 255)
                                } else {
                                    Color32::WHITE
                                })
                                .stroke(Stroke::new(
                                    1.0_f32,
                                    if selected {
                                        Color32::from_rgb(204, 215, 255)
                                    } else {
                                        LINE
                                    },
                                ))
                                .corner_radius(12)
                                .inner_margin(12)
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.label(RichText::new(&row.title).strong().size(14.0));
                                    ui.label(
                                        RichText::new(if row.demo {
                                            "Демонстрация"
                                        } else if row.eligible {
                                            "Доступна проверка DPI"
                                        } else {
                                            "Только просмотр"
                                        })
                                        .size(11.0)
                                        .color(MUTED),
                                    );
                                })
                                .response
                                .interact(egui::Sense::click());
                            if response.clicked() && !self.busy {
                                action = Some(Action::Probe(index));
                            }
                            ui.add_space(4.0);
                        }
                    });
                ui.add_space(16.0);
                if ui
                    .add_enabled(
                        !self.busy,
                        egui::Button::new(if self.demo {
                            "Выйти из демо"
                        } else {
                            "Посмотреть демо"
                        }),
                    )
                    .clicked()
                {
                    action = Some(if self.demo {
                        Action::Scan
                    } else {
                        Action::Demo
                    });
                }
                ui.add_space(12.0);
                ui.label(
                    RichText::new("Без аккаунта. Без телеметрии.\nБез фоновой службы.")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        egui::TopBottomPanel::bottom("footer")
            .frame(egui::Frame::new().fill(BG).inner_margin(12))
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Прошивка, сброс и запись аппаратных профилей заблокированы")
                        .size(11.0)
                        .color(ACCENT),
                );
                ui.label(
                    RichText::new("Pet-проект · полностью написан нейросетью под вашим надзором")
                        .size(11.0)
                        .color(MUTED),
                );
            });
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(28)).show(ctx,|ui| {
            ui.horizontal(|ui| {
                ui.heading("Настройки");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center),|ui| {
                    if ui.add_enabled(!self.busy,egui::Button::new("Обновить")).clicked(){action=Some(if self.demo {Action::Demo}else{Action::Scan});}
                    if self.busy {ui.spinner();}
                });
            });
            ui.add_space(5.0);
            ui.label(RichText::new(if self.demo {"ДЕМО · никакие команды устройствам не отправляются"} else {"Изменения — только по вашему действию"}).size(12.0).color(if self.demo {ACCENT}else{MUTED}));
            ui.add_space(18.0);
            egui::ScrollArea::vertical().show(ui,|ui| {
                let Some(row)=self.selected.and_then(|i|self.rows.get(i)).cloned() else {
                    card().show(ui,|ui| {
                        ui.add_space(24.0);
                        ui.heading("Подключите мышь");
                        ui.label("Выберите устройство слева или обновите список после подключения.");
                        ui.add_space(14.0);
                        ui.label(RichText::new("Поддерживаемые возможности определяются по протоколу, а не только по названию модели.").color(MUTED));
                        ui.add_space(18.0);
                    });
                    self.safety_card(ui);
                    return;
                };
                ui.label(RichText::new(&row.title).size(21.0).strong());
                ui.label(RichText::new(&row.detail).size(11.0).color(MUTED));
                if ui.add_enabled(!self.busy,egui::Button::new("Сохранить диагностику").small()).clicked(){action=Some(Action::Diagnostic);}
                ui.add_space(16.0);
                let caps=self.caps.clone();
                ui.columns(2,|columns| {
                let ui=&mut columns[0];
                card().show(ui,|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.set_min_height(250.0);
                    ui.label(RichText::new("Чувствительность").strong().size(16.0));
                    ui.add_space(6.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(self.requested.to_string()).size(42.0).strong());
                        ui.label(RichText::new("DPI").size(14.0).color(MUTED));
                    });
                    if let Some(c)=&caps {
                        ui.add_enabled_ui(!self.busy,|ui| {
                            let mut index=c.values.iter().position(|v|*v==self.requested).unwrap_or(0);
                            if ui.add(egui::Slider::new(&mut index,0..=c.values.len()-1).show_value(false)).changed(){self.requested=c.values[index];}
                            ui.horizontal_wrapped(|ui| {
                                for value in [400,800,1200,1600,3200,6400].into_iter().filter(|v|c.values.contains(v)) {
                                    if ui.selectable_label(self.requested==value,value.to_string()).clicked(){self.requested=value;}
                                }
                            });
                        });
                        ui.add_space(8.0);
                        ui.label(RichText::new(format!("Сейчас: {} DPI · значения получены от {}",c.current_dpi,if c.demo {"симулятора"}else{"устройства"})).size(12.0).color(MUTED));
                        ui.add_space(8.0);
                        let enabled=!self.busy && self.requested!=c.current_dpi && c.values.contains(&self.requested);
                        if ui.add_enabled(enabled,egui::Button::new(RichText::new("Применить DPI").color(Color32::WHITE)).fill(ACCENT)).clicked(){action=Some(Action::Dpi);}
                    } else {
                        ui.label(RichText::new(if self.busy {"Проверяем возможности…"}else{&row.reason}).color(MUTED));
                        if row.eligible && ui.add_enabled(!self.busy,egui::Button::new("Проверить возможности")).clicked(){action=Some(Action::Probe(self.selected.unwrap()));}
                    }
                });
                let ui=&mut columns[1];
                card().show(ui,|ui| {
                    ui.set_min_width(ui.available_width());
                    ui.set_min_height(250.0);
                    ui.label(RichText::new("Подсветка").strong().size(16.0));
                    ui.add_space(6.0);
                    let palette_width=(ui.available_width()-110.0).max(90.0);
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.set_width(palette_width);
                            ui.horizontal_wrapped(|ui| {
                                for color in [[83,129,255],[106,205,181],[242,153,117],[197,143,239],[246,205,104]] {
                                    let (rect,response)=ui.allocate_exact_size(egui::vec2(27.0,27.0),egui::Sense::click());
                                    ui.painter().circle_filled(rect.center(),10.0,Color32::from_rgb(color[0],color[1],color[2]));
                                    if self.color==color {ui.painter().circle_stroke(rect.center(),13.0,Stroke::new(1.0_f32,INK));}
                                    if response.clicked(){self.color=color;}
                                }
                                ui.color_edit_button_srgb(&mut self.color);
                            });
                            ui.add_space(8.0);
                            ui.label(RichText::new("Палитра — только предпросмотр").size(12.0).color(MUTED));
                        });
                        mouse_preview(ui,self.color);
                    });
                    let enabled=caps.as_ref().is_some_and(|c|c.demo)&&!self.busy;
                    if ui.add_enabled(enabled,egui::Button::new("Применить подсветку")).clicked(){action=Some(Action::Rgb);}
                    ui.label(RichText::new(if enabled {"Изменяется только демонстрационная мышь."} else {"Запись отключена: безопасный протокол подсветки для устройства ещё не подтверждён."}).size(12.0).color(MUTED));
                });
                });
                ui.add_space(14.0);
                card().show(ui,|ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Профиль DPI").strong());
                        ui.add(egui::TextEdit::singleline(&mut self.profile_name).desired_width(140.0));
                        if ui.add_enabled(caps.is_some()&&!self.busy,egui::Button::new("Сохранить")).clicked(){action=Some(Action::Save);}
                        if ui.add_enabled(caps.is_some()&&!self.busy,egui::Button::new("Загрузить")).clicked(){action=Some(Action::Load);}
                    });
                    ui.label(RichText::new("Загрузка профиля не изменяет устройство автоматически.").size(11.0).color(MUTED));
                });
                ui.add_space(12.0);
                ui.label(RichText::new(&self.notice).size(12.0).color(if self.failed {Color32::from_rgb(163,91,52)}else{MUTED}));
                self.safety_card(ui);
            });
        });
        if !self.busy && ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            action = Some(if self.demo {
                Action::Demo
            } else {
                Action::Scan
            });
        }
        if let Some(action) = action {
            self.action(action);
        }
    }

    fn safety_card(&self, ui: &mut egui::Ui) {
        ui.add_space(16.0);
        egui::CollapsingHeader::new("Защита устройства: команды ограничены").show(ui,|ui| {
            ui.label("Прошивка, bootloader, сброс и запись аппаратных профилей недоступны.");
            ui.label("Перед DPI проверяются дескриптор и допустимые значения. После — ответ устройства.");
            ui.label("При ошибке нет повторов и автоматического отката. Нужна новая проверка.");
            ui.label(RichText::new("Абсолютный нулевой риск обещать нельзя: физическая проверка устройств ещё не проведена.").color(MUTED).size(12.0));
        });
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        self.poll();
        self.ui(ctx);
        if let Some(path) = self.capture.clone() {
            let screenshot = ctx.input(|i| {
                i.events.iter().find_map(|event| match event {
                    egui::Event::Screenshot { image, .. } => Some(image.clone()),
                    _ => None,
                })
            });
            if let Some(screenshot) = screenshot {
                let bytes: Vec<u8> = screenshot
                    .pixels
                    .iter()
                    .flat_map(|p| p.to_array())
                    .collect();
                let _ = image::save_buffer_with_format(
                    path,
                    &bytes,
                    screenshot.width() as u32,
                    screenshot.height() as u32,
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                );
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            } else if !self.busy && !self.capture_requested {
                self.capture_requested = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(Default::default()));
            }
        }
    }
}

fn card() -> egui::Frame {
    egui::Frame::new()
        .fill(Color32::WHITE)
        .stroke(Stroke::new(1.0_f32, LINE))
        .corner_radius(14)
        .inner_margin(18)
}

fn mouse_preview(ui: &mut egui::Ui, color: [u8; 3]) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(98.0, 82.0), egui::Sense::hover());
    let body = egui::Rect::from_center_size(rect.center(), egui::vec2(45.0, 76.0));
    ui.painter()
        .rect_filled(body, 22, Color32::from_rgb(234, 238, 247));
    ui.painter().rect_stroke(
        body,
        22,
        Stroke::new(1.5_f32, Color32::from_rgb(185, 195, 215)),
        egui::StrokeKind::Inside,
    );
    ui.painter().line_segment(
        [
            body.center_top() + egui::vec2(0.0, 2.0),
            body.center() + egui::vec2(0.0, -5.0),
        ],
        Stroke::new(1.0_f32, Color32::from_rgb(185, 195, 215)),
    );
    ui.painter().rect_filled(
        egui::Rect::from_center_size(
            body.center_top() + egui::vec2(0.0, 18.0),
            egui::vec2(5.0, 12.0),
        ),
        3,
        MUTED,
    );
    ui.painter().circle_filled(
        body.center() + egui::vec2(0.0, 17.0),
        4.0,
        Color32::from_rgb(color[0], color[1], color[2]),
    );
}

fn profile_path(name: &str) -> crate::error::Result<PathBuf> {
    if name.is_empty()
        || name.len() > 48
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(crate::error::Error::Invalid(
            "profile name must use letters, digits, - or _".into(),
        ));
    }
    Ok(std::env::current_exe()
        .map_err(|_| crate::error::Error::Io("application directory".into()))?
        .parent()
        .ok_or(crate::error::Error::Io("application directory".into()))?
        .join("profiles")
        .join(format!("{name}.json")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn profile_names_cannot_escape_portable_directory() {
        for name in ["..", "../config", "C:\\file", "a/b", "a.b", ""] {
            assert!(profile_path(name).is_err());
        }
        assert!(profile_path("my-profile")
            .unwrap()
            .ends_with("profiles/my-profile.json"));
    }

    fn ready(app: &mut DesktopApp) {
        for _ in 0..200 {
            app.poll();
            if !app.busy {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        panic!("demo worker timed out");
    }

    #[test]
    fn draft_changes_do_not_apply_until_explicit_action() {
        let ctx = egui::Context::default();
        let mut app = DesktopApp::new(&ctx, true, None).unwrap();
        ready(&mut app);
        app.requested = 1600;
        let _ = ctx.run(egui::RawInput::default(), |ctx| app.ui(ctx));
        assert_eq!(app.caps.as_ref().unwrap().current_dpi, 800);
        assert!(!app.busy);
        app.action(Action::Dpi);
        ready(&mut app);
        assert_eq!(app.caps.as_ref().unwrap().current_dpi, 1600);
    }
}
