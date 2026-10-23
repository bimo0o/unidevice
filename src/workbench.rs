use super::*;
use crate::{
    compatibility::{Adapter, Pack},
    effects::{Easing, Effect, Keyframe},
    portable::PortableProfile,
    sensitivity::Feel,
};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Page {
    #[default]
    Device,
    Effects,
    Scene,
    Profiles,
    Sensitivity,
    Compatibility,
}
impl Page {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Device => "Устройство",
            Self::Effects => "Эффекты",
            Self::Scene => "Сцена",
            Self::Profiles => "Профили",
            Self::Sensitivity => "Чувствительность",
            Self::Compatibility => "Совместимость",
        }
    }
}
#[derive(Clone)]
pub(super) struct Choice {
    pub selected: bool,
    pub brightness: f32,
    pub phase_ms: i32,
}
pub(super) struct Workbench {
    pub page: Page,
    pub keyframes: Vec<Keyframe>,
    pub easing: Easing,
    pub preview: bool,
    pub preset_name: String,
    pub import_path: String,
    pub loaded: Option<PortableProfile>,
    pub include_effect: bool,
    pub include_feel: bool,
    pub feel: Feel,
    pub desired_cm: f64,
    pub measuring: bool,
    pub raw: [f64; 2],
    pub raw_events: u64,
    pub distance_cm: f64,
    pub choices: Vec<Choice>,
    pub scene_running: bool,
    pub scene_status: Vec<desktop::SceneStatus>,
    pub packages: Vec<Pack>,
    pub draft_pack: Option<Pack>,
    pub message: String,
}
impl Default for Workbench {
    fn default() -> Self {
        Self {
            page: Page::Device,
            keyframes: Effect::default().keyframes,
            easing: Easing::Smooth,
            preview: false,
            preset_name: "my-effect".into(),
            import_path: String::new(),
            loaded: None,
            include_effect: true,
            include_feel: false,
            feel: Feel {
                measured_cm360: 40.0,
                reference_dpi: 800,
                game_sensitivity: 1.0,
            },
            desired_cm: 40.0,
            measuring: false,
            raw: [0.0; 2],
            raw_events: 0,
            distance_cm: 10.0,
            choices: Vec::new(),
            scene_running: false,
            scene_status: Vec::new(),
            packages: crate::compatibility::installed(),
            draft_pack: None,
            message: String::new(),
        }
    }
}

impl DesktopApp {
    pub(super) fn effect_draft(&self) -> Effect {
        Effect {
            pattern: self.pattern,
            first: self.color,
            second: self.second_color,
            seconds: self.effect_seconds,
            easing: self.tools.easing,
            keyframes: self.tools.keyframes.clone(),
        }
    }
    pub(super) fn set_effect_draft(&mut self, e: Effect) {
        self.pattern = e.pattern;
        self.color = e.first;
        self.second_color = e.second;
        self.effect_seconds = e.seconds;
        self.tools.easing = e.easing;
        if !e.keyframes.is_empty() {
            self.tools.keyframes = e.keyframes;
        }
    }
    pub(super) fn transfer_file(&mut self, save: bool) -> crate::error::Result<String> {
        if save {
            let caps = self
                .caps
                .as_ref()
                .ok_or(crate::error::Error::Invalid("select a device first".into()))?;
            let p = PortableProfile {
                version: 2,
                dpi: Some(self.requested),
                rgb: caps.rgb_control.then_some(self.color),
                effect: (caps.rgb_control && self.tools.include_effect)
                    .then(|| self.effect_draft()),
                feel: self.tools.include_feel.then(|| self.tools.feel.clone()),
            };
            crate::storage::save_new(&profile_path(&self.profile_name)?, &p.json()?)?;
            Ok(
                "Переносимый профиль сохранён: profiles. Существующие файлы не перезаписываются."
                    .into(),
            )
        } else {
            self.tools.loaded = None;
            let path = if self.tools.import_path.trim().is_empty() {
                profile_path(&self.profile_name)?
            } else {
                PathBuf::from(self.tools.import_path.trim())
            };
            let text = crate::storage::read(&path)?;
            let p = PortableProfile::from_any_json(&text)?;
            if let Some(dpi) = p.dpi {
                self.requested = dpi;
            }
            if let Some(rgb) = p.rgb {
                self.color = rgb;
            }
            if let Some(effect) = &p.effect {
                self.set_effect_draft(effect.clone());
            }
            if let Some(feel) = &p.feel {
                self.tools.feel = feel.clone();
                self.tools.include_feel = true;
            }
            self.tools.loaded = Some(p);
            Ok("Профиль загружен только в черновик. Проверьте совместимость и примените нужные поля.".into())
        }
    }
    pub(super) fn process_raw_motion(&mut self, ctx: &egui::Context) {
        if self.tools.page != Page::Sensitivity {
            self.tools.measuring = false;
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F8)) {
            if !self.tools.measuring {
                self.tools.raw = [0.0; 2];
                self.tools.raw_events = 0;
            }
            self.tools.measuring = !self.tools.measuring;
        }
        if !ctx.input(|i| i.focused) {
            self.tools.measuring = false;
        }
        if self.tools.measuring {
            ctx.input(|i| {
                for e in &i.events {
                    if let egui::Event::MouseMoved(delta) = e {
                        self.tools.raw[0] += f64::from(delta.x);
                        self.tools.raw[1] += f64::from(delta.y);
                        self.tools.raw_events += 1;
                    }
                }
            });
        }
    }
    pub(super) fn timeline_ui(&mut self, ui: &mut egui::Ui) {
        if self.pattern == crate::effects::Pattern::Timeline {
            ui.label(RichText::new("Цветовые точки · цикл замыкается плавно").small());
            let mut remove = None;
            let len = self.tools.keyframes.len();
            for i in 0..len {
                ui.push_id(i, |ui| {
                    ui.horizontal(|ui| {
                        ui.color_edit_button_srgb(&mut self.tools.keyframes[i].color);
                        let lower = if i == 0 {
                            0.0
                        } else {
                            self.tools.keyframes[i - 1].at + 0.02
                        };
                        let upper = if i + 1 == len {
                            0.98
                        } else {
                            self.tools.keyframes[i + 1].at - 0.02
                        };
                        let mut percent = self.tools.keyframes[i].at * 100.0;
                        if ui
                            .add_enabled(
                                i > 0,
                                egui::DragValue::new(&mut percent)
                                    .range(lower * 100.0..=upper * 100.0)
                                    .speed(0.5)
                                    .suffix(" %"),
                            )
                            .changed()
                        {
                            self.tools.keyframes[i].at = percent / 100.0;
                        }
                        if i > 0 && len > 2 && ui.small_button("Убрать").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(index) = remove {
                self.tools.keyframes.remove(index);
            }
            if len < 8 && ui.button("Добавить цветовую точку").clicked() {
                let (index, _) = self
                    .tools
                    .keyframes
                    .iter()
                    .enumerate()
                    .map(|(i, k)| {
                        (
                            i,
                            self.tools.keyframes.get(i + 1).map_or(1.0, |n| n.at) - k.at,
                        )
                    })
                    .max_by(|a, b| a.1.total_cmp(&b.1))
                    .unwrap();
                let at = (self.tools.keyframes[index].at
                    + self.tools.keyframes.get(index + 1).map_or(1.0, |k| k.at))
                    * 0.5;
                self.tools.keyframes.insert(
                    index + 1,
                    Keyframe {
                        at,
                        color: self.second_color,
                    },
                );
            }
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tools.easing, Easing::Smooth, "Плавные переходы");
                ui.selectable_value(&mut self.tools.easing, Easing::Linear, "Линейные");
            });
        }
        let effect = self.effect_draft();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 22.0), egui::Sense::hover());
        for n in 0..80 {
            let c = effect.color(std::time::Duration::from_secs_f64(
                f64::from(effect.seconds) * f64::from(n) / 80.0,
            ));
            ui.painter().rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(rect.left() + rect.width() * n as f32 / 80.0, rect.top()),
                    egui::pos2(
                        rect.left() + rect.width() * (n + 1) as f32 / 80.0 + 0.5,
                        rect.bottom(),
                    ),
                ),
                0,
                Color32::from_rgb(c[0], c[1], c[2]),
            );
        }
        ui.checkbox(
            &mut self.tools.preview,
            "Анимированный предпросмотр без записи",
        );
        if self.tools.preview {
            let c = effect.color(std::time::Duration::from_secs_f64(
                ui.input(|i| i.time).max(0.0),
            ));
            let (rect, _) = ui
                .allocate_exact_size(egui::vec2(ui.available_width(), 12.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(rect, 4, Color32::from_rgb(c[0], c[1], c[2]));
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(33));
        }
        egui::CollapsingHeader::new("Сохранить / импортировать эффект").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Имя:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.tools.preset_name).desired_width(135.0),
                );
            });
            ui.horizontal(|ui| {
                if ui.button("Сохранить эффект").clicked() {
                    self.tools.message = match (|| {
                        let json = serde_json::to_string_pretty(&self.effect_draft())
                            .map_err(|_| crate::error::Error::Io("effect JSON".into()))?;
                        if !self.effect_draft().valid() {
                            return Err(crate::error::Error::Invalid("effect points".into()));
                        }
                        crate::storage::save_new(
                            &crate::storage::path("effects", &self.tools.preset_name)?,
                            &json,
                        )
                    })() {
                        Ok(()) => {
                            "Эффект сохранён в effects; файл можно передать другому пользователю."
                                .into()
                        }
                        Err(e) => e.to_string(),
                    };
                }
                if ui.button("Загрузить по имени").clicked() {
                    self.tools.message = match (|| {
                        let text = crate::storage::read(&crate::storage::path(
                            "effects",
                            &self.tools.preset_name,
                        )?)?;
                        let e: Effect = serde_json::from_str(&text)
                            .map_err(|_| crate::error::Error::Invalid("effect JSON".into()))?;
                        if !e.valid() {
                            return Err(crate::error::Error::Invalid("effect values".into()));
                        }
                        Ok(e)
                    })() {
                        Ok(e) => {
                            self.set_effect_draft(e);
                            "Эффект загружен в предпросмотр; устройство не изменено.".into()
                        }
                        Err(e) => e.to_string(),
                    };
                }
            });
            ui.label(
                RichText::new("Поместите полученный JSON в папку effects рядом с программой.")
                    .small(),
            );
        });
    }
    pub(super) fn workbench_ui(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        match self.tools.page {
            Page::Effects => self.effects_ui(ui, action),
            Page::Scene => self.scene_ui(ui, action),
            Page::Profiles => self.profiles_ui(ui, action),
            Page::Sensitivity => self.sensitivity_ui(ui),
            Page::Compatibility => self.compatibility_ui(ui, action),
            Page::Device => {}
        }
        if !self.tools.message.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new(&self.tools.message).color(MUTED));
        }
        ui.add_space(10.0);
        ui.label(RichText::new(&self.notice).small().color(if self.failed {
            Color32::from_rgb(163, 91, 52)
        } else {
            MUTED
        }));
    }
    fn effects_ui(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.heading("Конструктор эффектов");
        ui.label("Редактирование и предпросмотр не меняют устройство. Эффект применяется отдельной кнопкой.");
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    self.caps.as_ref().is_some_and(|c| c.rgb_control)
                        && !self.busy
                        && self.effect_draft().valid(),
                    egui::Button::new(if self.effect_running {
                        "Обновить запущенный эффект"
                    } else {
                        "Запустить на выбранном устройстве"
                    }),
                )
                .clicked()
            {
                *action = Some(Action::StartEffect);
            }
            if ui
                .add_enabled(
                    self.effect_running && !self.busy,
                    egui::Button::new("Остановить"),
                )
                .clicked()
            {
                *action = Some(Action::StopEffect);
            }
        });
        ui.horizontal_wrapped(|ui| {
            for pattern in [
                crate::effects::Pattern::Gradient,
                crate::effects::Pattern::Breathing,
                crate::effects::Pattern::Rainbow,
                crate::effects::Pattern::Timeline,
            ] {
                ui.selectable_value(&mut self.pattern, pattern, pattern.label());
            }
        });
        ui.horizontal(|ui| {
            ui.label("Основные цвета:");
            ui.color_edit_button_srgb(&mut self.color);
            ui.color_edit_button_srgb(&mut self.second_color);
        });
        ui.add(egui::Slider::new(&mut self.effect_seconds, 4.0..=30.0).suffix(" секунд / цикл"));
        self.timeline_ui(ui);
        ui.label(
            RichText::new("Для нескольких устройств используйте вкладку «Общая сцена».")
                .small()
                .color(MUTED),
        );
    }
    fn scene_ui(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.heading("Одна сцена для нескольких устройств");
        ui.label("Общие цвета и единые часы. Запуск только вручную; доступность RGB проверяется до первого кадра.");
        self.tools.choices.resize(
            self.rows.len(),
            Choice {
                selected: false,
                brightness: 1.0,
                phase_ms: 0,
            },
        );
        for (i, row) in self.rows.iter().enumerate().filter(|(_, r)| r.eligible) {
            card().show(ui, |ui| {
                ui.checkbox(&mut self.tools.choices[i].selected, &row.title);
                ui.horizontal(|ui| {
                    ui.add(
                        egui::Slider::new(&mut self.tools.choices[i].brightness, 0.0..=1.0)
                            .text("яркость"),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.tools.choices[i].phase_ms)
                            .range(-500..=500)
                            .suffix(" мс фаза"),
                    );
                });
            });
            ui.add_space(6.0);
        }
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("Запустить общую сцену"))
                .clicked()
            {
                *action = Some(Action::StartScene);
            }
            if ui
                .add_enabled(
                    self.tools.scene_running && !self.busy,
                    egui::Button::new("Остановить сцену"),
                )
                .clicked()
            {
                *action = Some(Action::StopScene);
            }
        });
        for status in &self.tools.scene_status {
            ui.horizontal(|ui| {
                ui.colored_label(
                    Color32::from_rgb(status.color[0], status.color[1], status.color[2]),
                    "●",
                );
                ui.label(format!("{} · {} DPI", status.name, status.dpi));
            });
        }
        ui.label(RichText::new("Каждое устройство получает кадры в отдельном последовательном канале, чтобы сохранить плавность. Часы общие; фазой можно компенсировать задержку. Аппаратной атомарности нет.").small().color(MUTED));
        egui::CollapsingHeader::new("Редактор общей сцены")
            .default_open(true)
            .show(ui, |ui| {
                for p in [
                    crate::effects::Pattern::Gradient,
                    crate::effects::Pattern::Breathing,
                    crate::effects::Pattern::Rainbow,
                    crate::effects::Pattern::Timeline,
                ] {
                    ui.selectable_value(&mut self.pattern, p, p.label());
                }
                ui.horizontal(|ui| {
                    ui.label("Цвета:");
                    ui.color_edit_button_srgb(&mut self.color);
                    ui.color_edit_button_srgb(&mut self.second_color);
                });
                ui.add(
                    egui::Slider::new(&mut self.effect_seconds, 4.0..=30.0).text("секунд / цикл"),
                );
                self.timeline_ui(ui);
            });
    }
    fn profiles_ui(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.heading("Перенос настроек на другую мышь");
        ui.label("Сохраните профиль, выберите новое устройство слева, загрузите профиль и проверьте совместимость.");
        ui.horizontal(|ui| {
            ui.label("Имя файла:");
            ui.text_edit_singleline(&mut self.profile_name);
        });
        ui.checkbox(&mut self.tools.include_effect, "Включить черновик эффекта");
        ui.checkbox(
            &mut self.tools.include_feel,
            "Включить введённый эталон чувствительности",
        );
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.caps.is_some() && !self.busy,
                    egui::Button::new("Сохранить профиль"),
                )
                .clicked()
            {
                *action = Some(Action::Save);
            }
            if ui
                .add_enabled(!self.busy, egui::Button::new("Загрузить"))
                .clicked()
            {
                *action = Some(Action::Load);
            }
        });
        ui.label("Внешний JSON (необязательно; пусто = profiles/имя.json):");
        ui.text_edit_singleline(&mut self.tools.import_path);
        if let Some(p) = &self.tools.loaded {
            ui.separator();
            ui.heading("Проверка перед переносом");
            if let Some(dpi) = p.dpi {
                let supported = self.caps.as_ref().is_some_and(|c| c.values.contains(&dpi));
                ui.label(format!(
                    "DPI профиля: {dpi} · {}",
                    if supported {
                        "точно поддерживается"
                    } else {
                        "не входит в допустимые значения выбранного устройства"
                    }
                ));
                if let Some(nearest) = self
                    .caps
                    .as_ref()
                    .and_then(|c| crate::portable::nearest(&c.values, f64::from(dpi)))
                {
                    if !supported
                        && ui
                            .button(format!("Выбрать ближайшее: {nearest} DPI"))
                            .clicked()
                    {
                        self.requested = nearest;
                    }
                }
            }
            if p.rgb.is_some() {
                ui.label(if self.caps.as_ref().is_some_and(|c| c.rgb_control) {
                    "Цвет можно перенести"
                } else {
                    "Цвет будет пропущен: RGB недоступно"
                });
            }
            if p.effect.is_some() {
                ui.label("Эффект перенесён в редактор. Запускается отдельной кнопкой.");
            }
            if p.feel.is_some() {
                ui.label("Эталон чувствительности доступен во вкладке «Чувствительность».");
            }
            let applicable = self.caps.as_ref().is_some_and(|c| {
                p.dpi.is_some() && c.values.contains(&self.requested)
                    || p.rgb.is_some() && c.rgb_control
            });
            if ui
                .add_enabled(
                    applicable && !self.busy,
                    egui::Button::new("Применить совместимые поля"),
                )
                .clicked()
            {
                *action = Some(Action::Transfer);
            }
        }
        if let Some(c) = &self.caps {
            ui.label(format!(
                "Выбранное устройство сейчас: {} DPI. В черновике: {} DPI.",
                c.current_dpi, self.requested
            ));
        }
        ui.label(RichText::new("Загрузка, подбор ближайшего значения и редактирование ничего не записывают в устройство.").small().color(MUTED));
    }
    fn sensitivity_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Мастер одинаковой чувствительности");
        if let Some(caps) = &self.caps {
            if ui
                .button(format!("Взять текущие {} DPI как эталон", caps.current_dpi))
                .clicked()
            {
                self.tools.feel.reference_dpi = caps.current_dpi;
            }
        }
        ui.label("Для другой игры измерьте текущие см/360° в ней и пересчитайте к сохранённому эталону. Программа не угадывает коэффициенты игр и не меняет их файлы.");
        ui.label("Для одной и той же игры: сохранение DPI × sensitivity. Между играми этот коэффициент может отличаться.");
        ui.horizontal(|ui| {
            ui.label("DPI эталонной мыши:");
            ui.add(egui::DragValue::new(&mut self.tools.feel.reference_dpi).range(100..=25600));
        });
        ui.horizontal(|ui| {
            ui.label("Чувствительность в игре:");
            ui.add(
                egui::DragValue::new(&mut self.tools.feel.game_sensitivity)
                    .range(0.0001..=1000.0)
                    .speed(0.01),
            );
        });
        if let Ok(s) = self.tools.feel.sensitivity_at(self.requested) {
            ui.label(
                RichText::new(format!(
                    "При {} DPI выставьте в той же игре: {:.5}",
                    self.requested, s
                ))
                .strong(),
            );
        }
        ui.separator();
        ui.label("Измерьте линейкой расстояние для полного оборота в игре:");
        ui.horizontal(|ui| {
            ui.label("Сейчас, см / 360°:");
            ui.add(
                egui::DragValue::new(&mut self.tools.feel.measured_cm360)
                    .range(1.0..=300.0)
                    .speed(0.1),
            );
            ui.label("Хочу:");
            ui.add(
                egui::DragValue::new(&mut self.tools.desired_cm)
                    .range(1.0..=300.0)
                    .speed(0.1),
            );
        });
        if let Ok(dpi) = self.tools.feel.dpi_for_distance(self.tools.desired_cm) {
            ui.label(format!(
                "Расчёт при неизменной чувствительности игры: {dpi:.1} DPI."
            ));
            if let Some(nearest) = self
                .caps
                .as_ref()
                .and_then(|c| crate::portable::nearest(&c.values, dpi))
            {
                if ui
                    .button(format!("Перенести {nearest} DPI в черновик"))
                    .clicked()
                {
                    self.requested = nearest;
                    self.tools.message =
                        "Число выбрано. Для записи нажмите «Применить DPI» во вкладке устройства."
                            .into();
                }
            }
        }
        ui.checkbox(
            &mut self.tools.include_feel,
            "Добавлять этот эталон в переносимый профиль",
        );
        egui::CollapsingHeader::new("Измерение по необработанному движению · экспериментально").show(ui,|ui|{
            ui.label("F8: начало / остановка. Проведите одну мышь прямо на известное расстояние, затем нажмите F8, не двигая её к кнопке.");
            ui.add(egui::DragValue::new(&mut self.tools.distance_cm).range(1.0..=100.0).suffix(" см"));
            ui.label(if self.tools.measuring{"Идёт измерение… F8 для остановки"}else{"Измерение остановлено. F8 для нового прохода"});
            ui.label(format!("Суммарный вектор: {:.0}, {:.0}; событий: {}",self.tools.raw[0],self.tools.raw[1],self.tools.raw_events));
            let estimate=self.tools.raw[0].hypot(self.tools.raw[1])*2.54/self.tools.distance_cm;
            if self.tools.raw_events>0&&!self.tools.measuring {ui.label(format!("Оценка: {estimate:.1} единиц движения / дюйм."));}
            ui.label(RichText::new("Это оценка, не сертифицированный DPI: ОС/драйвер могут использовать другие единицы. События нескольких мышей не разделяются. Окно должно оставаться в фокусе; при потере фокуса измерение остановится. Сохраняются только счётчики в памяти, история ввода не записывается.").small().color(MUTED));
        });
    }
    fn compatibility_ui(&mut self, ui: &mut egui::Ui, action: &mut Option<Action>) {
        ui.heading("Автоадаптация и пакеты совместимости");
        ui.label("Название модели не даёт разрешения на запись. Сначала проверяются дескриптор и ответы известного протокола.");
        if let Some(row) = self.selected.and_then(|i| self.rows.get(i)).cloned() {
            card().show(ui, |ui| {
                ui.strong(&row.title);
                ui.label(&row.reason);
                if let Some(c) = &self.caps {
                    let p = c.passport();
                    ui.label(format!("Адаптер: {}", p.adapter));
                    ui.label(format!(
                        "Прошивка: {}",
                        p.firmware
                            .as_deref()
                            .unwrap_or("не запрашивается этим адаптером")
                    ));
                    ui.label(&p.evidence);
                    ui.label(format!("DPI: {}", p.dpi));
                    ui.label(format!("RGB: {}", p.rgb));
                    for s in &p.safety {
                        ui.label(s);
                    }
                }
                if let Some(pack) = &row.package {
                    ui.label(format!(
                        "Пакет: {} · сведения автора пакета, не независимая проверка",
                        pack.id
                    ));
                    ui.label(&pack.evidence);
                }
                if ui.button("Экспортировать паспорт").clicked() {
                    self.tools.message = match self.export_passport(&row) {
                        Ok(name) => format!("Паспорт сохранён: {name}"),
                        Err(e) => e.to_string(),
                    };
                }
                if ui
                    .add_enabled(
                        !self.busy,
                        egui::Button::new("Сохранить диагностику дескриптора"),
                    )
                    .clicked()
                {
                    *action = Some(Action::Diagnostic);
                }
            });
            ui.add_space(8.0);
            if ui
                .button("Подготовить пакет для выбранной модели")
                .clicked()
            {
                self.tools.message=match row.identity(){Some((vid,pid))if vid==0x046d||(vid,pid)==(0x1b1c,0x1b75)=>{self.tools.draft_pack=Some(Pack{version:1,revision:row.package.as_ref().map_or(1,|p|p.revision.saturating_add(1)),id:"my-device".into(),name:row.title.clone(),adapter:if vid==0x046d{Adapter::Hidpp2}else{Adapter::Harpoon313},vid,pids:vec![pid],max_dpi:None,evidence:"Local user description; hardware support must be checked by the compiled adapter".into(),sources:vec![]});"Шаблон готов. Пакет не расширяет список аппаратных команд.".into()},_=>"Неизвестный протокол: сначала экспортируйте диагностику и проведите захват трафика. Пакет не может изобрести безопасный адаптер.".into()};
            }
        } else {
            ui.label("Выберите устройство слева, чтобы открыть паспорт и мастер диагностики.");
        }
        if let Some(pack) = self.tools.draft_pack.as_mut() {
            ui.separator();
            ui.label("ID файла:");
            ui.text_edit_singleline(&mut pack.id);
            ui.horizontal(|ui| {
                ui.label("Ревизия пакета:");
                ui.add(egui::DragValue::new(&mut pack.revision).range(1..=u32::MAX));
            });
            ui.label("Название:");
            ui.text_edit_singleline(&mut pack.name);
            ui.label("Источник и результаты проверки:");
            ui.text_edit_multiline(&mut pack.evidence);
            if ui.button("Установить созданный пакет").clicked() {
                self.tools.message = match pack.json().and_then(|text| Pack::install(&text)) {
                    Ok(name) => {
                        self.tools.packages = crate::compatibility::installed();
                        *action = Some(if self.demo {
                            Action::Demo
                        } else {
                            Action::Scan
                        });
                        format!("Пакет {name} установлен")
                    }
                    Err(e) => e.to_string(),
                };
            }
        }
        ui.separator();
        ui.label("Импорт пакета JSON с диска:");
        ui.text_edit_singleline(&mut self.tools.import_path);
        if ui.button("Проверить и установить пакет").clicked() {
            self.tools.message =
                match crate::storage::read(std::path::Path::new(self.tools.import_path.trim()))
                    .and_then(|s| Pack::install(&s))
                {
                    Ok(name) => {
                        self.tools.packages = crate::compatibility::installed();
                        *action = Some(if self.demo {
                            Action::Demo
                        } else {
                            Action::Scan
                        });
                        format!("Установлен пакет: {name}")
                    }
                    Err(e) => e.to_string(),
                };
        }
        ui.label(RichText::new("Только локальный импорт. Подпись издателя не проверяется. Пакет может назвать модель и сузить диапазон DPI, но не добавляет код, команды или поддержку новой прошивки. Произвольные поля в JSON отклоняются.").small().color(MUTED));
        for pack in &self.tools.packages {
            ui.label(format!(
                "• {} / {} / {:?}",
                pack.name, pack.id, pack.adapter
            ));
        }
    }
    fn export_passport(&self, row: &DeviceRow) -> crate::error::Result<String> {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let name = format!("passport-{stamp}");
        let value = serde_json::json!({"version":1,"app_version":env!("CARGO_PKG_VERSION"),"device_name":row.title,"identity":row.identity(),"descriptor_candidate":row.eligible,"reason":row.reason,"capabilities":self.caps.as_ref().map(|c|c.passport()),"package_claim":row.package,"serials_or_paths_included":false});
        crate::storage::save_new(
            &crate::storage::path("passports", &name)?,
            &serde_json::to_string_pretty(&value)
                .map_err(|_| crate::error::Error::Io("passport JSON".into()))?,
        )?;
        Ok(format!("passports/{name}.json"))
    }
}
