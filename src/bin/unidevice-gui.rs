#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let demo = args.iter().any(|s| s == "--demo");
    let capture = args
        .windows(2)
        .find(|p| p[0] == "--capture")
        .map(|p| std::path::PathBuf::from(&p[1]));
    if capture.is_some() && !demo {
        return;
    }
    if let Err(error) = unidevice::gui::run(demo, capture) {
        #[cfg(windows)]
        {
            let message:Vec<u16>="Не удалось открыть окно UniDevice. Проверьте драйвер графики. CLI доступен отдельным файлом unidevice.exe.\0".encode_utf16().collect();
            let title: Vec<u16> = "UniDevice\0".encode_utf16().collect();
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW(
                    std::ptr::null_mut(),
                    message.as_ptr(),
                    title.as_ptr(),
                    windows_sys::Win32::UI::WindowsAndMessaging::MB_OK
                        | windows_sys::Win32::UI::WindowsAndMessaging::MB_ICONERROR,
                );
            }
            let _ = error;
        }
        #[cfg(not(windows))]
        eprintln!("UniDevice startup failed: {error}");
        std::process::exit(1);
    }
}
