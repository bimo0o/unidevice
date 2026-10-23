#[derive(Clone, Debug, serde::Serialize)]
pub struct Passport {
    pub adapter: String,
    pub firmware: Option<String>,
    pub dpi: String,
    pub rgb: String,
    pub evidence: String,
    pub safety: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb_capability_does_not_inherit_harpoon_firmware_or_hardware_evidence() {
        let logitech = Passport::for_caps(Adapter::Logitech, true, "device data");
        assert!(logitech.firmware.is_none());
        assert!(logitech.adapter.contains("Logitech"));
        assert!(!logitech.evidence.contains("проверен на Harpoon"));
        let harpoon = Passport::for_caps(Adapter::Harpoon, true, "policy");
        assert_eq!(harpoon.firmware.as_deref(), Some("3.13"));
    }
}
#[derive(Clone, Copy, Debug)]
pub enum Adapter {
    Simulation,
    Harpoon,
    Logitech,
}
impl Passport {
    pub fn for_caps(adapter: Adapter, rgb: bool, dpi_source: &str) -> Self {
        let demo = matches!(adapter, Adapter::Simulation);
        let harpoon = matches!(adapter, Adapter::Harpoon);
        Self {
            adapter:if demo {"Simulation"}else if harpoon {"Corsair Harpoon feature64"}else{"Logitech HID++2 / DPI 2201/2202; RGB 8070 при наличии"}.into(),
            firmware:if harpoon {Some("3.13".into())}else{None},
            dpi:dpi_source.into(),rgb:if rgb {"Статический цвет, градиент, дыхание, радуга, таймлайн"}else{"Нет проверенного RGB-адаптера"}.into(),
            evidence:if demo {"Симулятор: физических подтверждений нет"}else if harpoon {"Протокол проверен на Harpoon 1B75 / 3.13: DPI, цвет и 31,5 кадра/с. Это не сертификация каждого экземпляра."}else{"Возможности получены от устройства; физический статус модели не подтверждён"}.into(),
            safety:vec!["Прошивка, bootloader, сброс, flash и сохранение аппаратного профиля заблокированы".into(),"При ошибке запись прекращается без повтора и автопереподключения".into()],
        }
    }
}
