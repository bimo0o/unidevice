#[derive(Clone, Debug, serde::Serialize)]
pub struct Passport {
    pub adapter: String,
    pub firmware: Option<String>,
    pub dpi: String,
    pub rgb: String,
    pub evidence: String,
    pub safety: Vec<String>,
}
impl Passport {
    pub fn for_caps(demo: bool, rgb: bool, dpi_source: &str) -> Self {
        Self {
            adapter:if demo {"Simulation"}else if rgb {"Corsair Harpoon feature64"}else{"Logitech HID++2 / 0x2201 или 0x2202"}.into(),
            firmware:if !demo&&rgb {Some("3.13".into())}else{None},
            dpi:dpi_source.into(),rgb:if rgb {"Статический цвет, градиент, дыхание, радуга, таймлайн"}else{"Нет проверенного RGB-адаптера"}.into(),
            evidence:if demo {"Симулятор: физических подтверждений нет"}else if rgb {"Протокол проверен на Harpoon 1B75 / 3.13: DPI, цвет и 31,5 кадра/с. Это не сертификация каждого экземпляра."}else{"Возможности получены от устройства; физический статус модели не подтверждён"}.into(),
            safety:vec!["Прошивка, bootloader, сброс, flash и сохранение аппаратного профиля заблокированы".into(),"При ошибке запись прекращается без повтора и автопереподключения".into()],
        }
    }
}
