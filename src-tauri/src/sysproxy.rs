use crate::error::{AppError, AppResult};

#[cfg(windows)]
pub fn set_enabled(enabled: bool, mixed_port: u16) -> AppResult<()> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::Networking::WinInet::{
        InternetSetOptionW, INTERNET_OPTION_REFRESH, INTERNET_OPTION_SETTINGS_CHANGED,
    };
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegSetValueExW, HKEY_CURRENT_USER, KEY_SET_VALUE, REG_DWORD,
    };

    const KEY_PATH: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings\0";

    unsafe {
        let mut key = std::ptr::null_mut();
        let path: Vec<u16> = KEY_PATH.encode_utf16().collect();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, path.as_ptr(), 0, KEY_SET_VALUE, &mut key);
        if status != ERROR_SUCCESS {
            return Err(AppError::from(format!("打开系统代理注册表失败: {status}")));
        }

        let enable: u32 = if enabled { 1 } else { 0 };
        let enable_name: Vec<u16> = "ProxyEnable\0".encode_utf16().collect();
        let status = RegSetValueExW(
            key,
            enable_name.as_ptr(),
            0,
            REG_DWORD,
            &enable as *const u32 as *const u8,
            std::mem::size_of::<u32>() as u32,
        );
        if status != ERROR_SUCCESS {
            RegCloseKey(key);
            return Err(AppError::from(format!("写入 ProxyEnable 失败: {status}")));
        }

        if enabled {
            let server = format!("127.0.0.1:{mixed_port}");
            set_sz(key, "ProxyServer", &server)?;
            set_sz(
                key,
                "ProxyOverride",
                "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;<local>",
            )?;
        }

        RegCloseKey(key);

        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_SETTINGS_CHANGED,
            std::ptr::null_mut(),
            0,
        );
        InternetSetOptionW(
            std::ptr::null_mut(),
            INTERNET_OPTION_REFRESH,
            std::ptr::null_mut(),
            0,
        );
    }

    Ok(())
}

#[cfg(windows)]
unsafe fn set_sz(
    key: windows_sys::Win32::System::Registry::HKEY,
    name: &str,
    value: &str,
) -> AppResult<()> {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{RegSetValueExW, REG_SZ};

    let name_w: Vec<u16> = format!("{name}\0").encode_utf16().collect();
    let mut value_w: Vec<u16> = value.encode_utf16().collect();
    value_w.push(0);
    let bytes = (value_w.len() * 2) as u32;
    let status = RegSetValueExW(
        key,
        name_w.as_ptr(),
        0,
        REG_SZ,
        value_w.as_ptr() as *const u8,
        bytes,
    );
    if status != ERROR_SUCCESS {
        return Err(AppError::from(format!("写入 {name} 失败: {status}")));
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool, _mixed_port: u16) -> AppResult<()> {
    Ok(())
}

pub fn disable() -> AppResult<()> {
    set_enabled(false, 0)
}
