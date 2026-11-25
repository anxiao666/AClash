use std::path::Path;
use std::process::Command;

use crate::error::{AppError, AppResult};

const RUN_VALUE: &str = "AClash";
const TASK_NAME: &str = "AClash";

#[cfg(windows)]
fn clear_run_key() {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, HKEY_CURRENT_USER, KEY_SET_VALUE,
    };

    unsafe {
        let mut hkey = std::ptr::null_mut();
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
            .encode_utf16()
            .collect();
        let status = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_SET_VALUE,
            &mut hkey,
        );
        if status != ERROR_SUCCESS {
            return;
        }
        let name: Vec<u16> = format!("{RUN_VALUE}\0").encode_utf16().collect();
        let _ = RegDeleteValueW(hkey, name.as_ptr());
        RegCloseKey(hkey);
    }
}

#[cfg(windows)]
fn run_key_enabled() -> bool {
    use windows_sys::Win32::Foundation::ERROR_SUCCESS;
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
    };

    unsafe {
        let mut hkey = std::ptr::null_mut();
        let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Run\0"
            .encode_utf16()
            .collect();
        let status = RegOpenKeyExW(HKEY_CURRENT_USER, subkey.as_ptr(), 0, KEY_READ, &mut hkey);
        if status != ERROR_SUCCESS {
            return false;
        }
        let name: Vec<u16> = format!("{RUN_VALUE}\0").encode_utf16().collect();
        let mut typ = 0u32;
        let mut size = 0u32;
        let query = RegQueryValueExW(
            hkey,
            name.as_ptr(),
            std::ptr::null_mut(),
            &mut typ,
            std::ptr::null_mut(),
            &mut size,
        );
        RegCloseKey(hkey);
        query == ERROR_SUCCESS && typ == REG_SZ
    }
}

#[cfg(windows)]
fn task_exists() -> bool {
    Command::new("schtasks")
        .args(["/Query", "/TN", TASK_NAME])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(windows)]
fn delete_task() {
    let _ = Command::new("schtasks")
        .args(["/Delete", "/TN", TASK_NAME, "/F"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(windows)]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Logon task with highest privileges — required for TUN after reboot.
#[cfg(windows)]
fn task_xml(exe: &Path) -> String {
    let command = xml_escape(&exe.display().to_string());
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>Start AClash elevated at user logon</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
    </Exec>
  </Actions>
</Task>
"#
    )
}

#[cfg(windows)]
fn create_elevated_logon_task() -> AppResult<()> {
    use crate::elevated;

    if !elevated::is_elevated() {
        return Err(AppError::from(
            "设置管理员开机自启需要以管理员身份运行 AClash",
        ));
    }

    let exe = std::env::current_exe().map_err(|e| AppError::from(e.to_string()))?;
    let xml = task_xml(&exe);
    let tmp = std::env::temp_dir().join("aclash-autostart-task.xml");
    // schtasks expects UTF-16 LE with BOM for /XML
    let mut utf16: Vec<u8> = vec![0xFF, 0xFE];
    for unit in xml.encode_utf16() {
        utf16.push((unit & 0xFF) as u8);
        utf16.push((unit >> 8) as u8);
    }
    std::fs::write(&tmp, utf16).map_err(|e| AppError::from(e.to_string()))?;

    delete_task();
    let status = Command::new("schtasks")
        .args([
            "/Create",
            "/TN",
            TASK_NAME,
            "/XML",
            &tmp.to_string_lossy(),
            "/F",
        ])
        .output()
        .map_err(|e| AppError::from(format!("创建计划任务失败：{e}")))?;
    let _ = std::fs::remove_file(&tmp);

    if !status.status.success() {
        let err = String::from_utf8_lossy(&status.stderr);
        let out = String::from_utf8_lossy(&status.stdout);
        let detail = if !err.trim().is_empty() {
            err.trim().to_string()
        } else {
            out.trim().to_string()
        };
        return Err(AppError::from(format!(
            "创建管理员开机自启失败：{detail}"
        )));
    }
    Ok(())
}

#[cfg(windows)]
pub fn is_enabled() -> bool {
    task_exists() || run_key_enabled()
}

#[cfg(not(windows))]
pub fn is_enabled() -> bool {
    false
}

#[cfg(windows)]
pub fn set_enabled(enabled: bool) -> AppResult<()> {
    if enabled {
        create_elevated_logon_task()?;
        // Drop the old non-elevated Run key so only the task remains.
        clear_run_key();
    } else {
        delete_task();
        clear_run_key();
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn set_enabled(_enabled: bool) -> AppResult<()> {
    Err(AppError::from("当前平台不支持开机自启"))
}
