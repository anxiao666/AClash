use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};
use crate::paths;
use crate::settings::AppSettings;

/// Hide console window for spawned Windows helpers (netstat/tasklist/etc).
#[cfg(windows)]
fn hide_console(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console(_cmd: &mut Command) {}

pub struct CoreState {
    pub child: Mutex<Option<Child>>,
    pub binary: Mutex<Option<PathBuf>>,
}

impl CoreState {
    pub fn new() -> Self {
        Self {
            child: Mutex::new(None),
            binary: Mutex::new(None),
        }
    }
}

pub fn set_binary(state: &CoreState, path: PathBuf) {
    if let Ok(mut guard) = state.binary.lock() {
        *guard = Some(path);
    }
}

pub fn is_running(state: &CoreState) -> bool {
    let mut guard = match state.child.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    match guard.as_mut() {
        Some(child) => matches!(child.try_wait(), Ok(None)),
        None => false,
    }
}

pub fn core_memory_bytes(state: &CoreState) -> Option<u64> {
    let mut guard = state.child.lock().ok()?;
    let child = guard.as_mut()?;
    if !matches!(child.try_wait(), Ok(None)) {
        return None;
    }
    let pid = child.id();
    memory_for_pid(pid)
}

#[cfg(windows)]
fn memory_for_pid(pid: u32) -> Option<u64> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            PageFaultCount: 0,
            PeakWorkingSetSize: 0,
            WorkingSetSize: 0,
            QuotaPeakPagedPoolUsage: 0,
            QuotaPagedPoolUsage: 0,
            QuotaPeakNonPagedPoolUsage: 0,
            QuotaNonPagedPoolUsage: 0,
            PagefileUsage: 0,
            PeakPagefileUsage: 0,
        };
        let ok = GetProcessMemoryInfo(
            handle,
            &mut counters,
            std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
        );
        CloseHandle(handle);
        if ok == 0 {
            None
        } else {
            Some(counters.WorkingSetSize as u64)
        }
    }
}

#[cfg(not(windows))]
fn memory_for_pid(_pid: u32) -> Option<u64> {
    None
}

fn controller_port(settings: &AppSettings) -> u16 {
    settings
        .external_controller
        .rsplit(':')
        .next()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(9090)
}

fn port_is_free(port: u16) -> bool {
    listening_pids(port).is_empty()
}

fn find_free_port(preferred: u16, candidates: &[u16]) -> AppResult<u16> {
    if port_is_free(preferred) {
        return Ok(preferred);
    }
    for port in candidates {
        if port_is_free(*port) {
            return Ok(*port);
        }
    }
    Err(AppError::from(format!(
        "找不到可用端口（首选 {preferred} 已被占用）"
    )))
}

fn is_mihomo_image(pid: u32) -> bool {
    process_image_name(pid)
        .map(|name| name.eq_ignore_ascii_case("mihomo.exe"))
        .unwrap_or(false)
}

/// Kill leftover mihomo for this data dir. Do not kill unrelated listeners
/// (another proxy may own the old default port).
fn cleanup_stale_cores(settings: &AppSettings, keep_pid: Option<u32>) {
    let Ok(data_dir) = paths::data_dir() else {
        return;
    };
    let data_key = data_dir.to_string_lossy().to_ascii_lowercase();
    let ports = [settings.mixed_port, controller_port(settings)];
    let mut doomed = mihomo_pids_using_data_dir(&data_key);
    for port in ports {
        for pid in listening_pids(port) {
            if is_mihomo_image(pid) && !doomed.contains(&pid) {
                doomed.push(pid);
            }
        }
    }
    for pid in doomed {
        if Some(pid) != keep_pid {
            let _ = kill_pid(pid);
        }
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        let left = mihomo_pids_using_data_dir(&data_key)
            .into_iter()
            .any(|pid| Some(pid) != keep_pid);
        if !left {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

fn claim_port(preferred: u16, candidates: &[u16], allow_relocate: bool) -> AppResult<u16> {
    if port_is_free(preferred) {
        return Ok(preferred);
    }
    if !allow_relocate {
        return Err(AppError::from(format!(
            "端口 {preferred} 已被其他程序占用，请关闭占用方或换一个端口"
        )));
    }
    find_free_port(preferred, candidates)
}

/// Ensure mixed / API ports can bind.
/// When `allow_relocate` is true (startup), busy preferred ports may move to a free fallback.
/// When false (user clicked apply), keep the chosen ports or return an error.
pub fn prepare_ports(settings: &mut AppSettings, allow_relocate: bool) -> AppResult<bool> {
    cleanup_stale_cores(settings, None);
    // Port may still show LISTENING briefly after kill.
    std::thread::sleep(std::time::Duration::from_millis(200));
    let mut changed = false;

    let mixed = claim_port(
        settings.mixed_port,
        &[17890, 17891, 17892, 7893, 7894, 7895, 10808, 10809],
        allow_relocate,
    )?;
    if mixed != settings.mixed_port {
        settings.mixed_port = mixed;
        changed = true;
    }

    let api = claim_port(
        controller_port(settings),
        &[19090, 19091, 19092, 9091, 9092, 9093, 9097],
        allow_relocate,
    )?;
    let want_ctrl = format!("127.0.0.1:{api}");
    if settings.external_controller.trim() != want_ctrl {
        settings.external_controller = want_ctrl;
        changed = true;
    }

    if settings.socks_port > 0 && !port_is_free(settings.socks_port) {
        return Err(AppError::from(format!(
            "Socks 端口 {} 已被占用",
            settings.socks_port
        )));
    }
    if settings.http_port > 0 && !port_is_free(settings.http_port) {
        return Err(AppError::from(format!(
            "Http 端口 {} 已被占用",
            settings.http_port
        )));
    }
    Ok(changed)
}

pub fn mixed_port_owned_by_core(app: &AppHandle, mixed_port: u16) -> bool {
    let state = app.state::<CoreState>();
    let Ok(guard) = state.child.lock() else {
        return false;
    };
    let Some(child) = guard.as_ref() else {
        return false;
    };
    let pid = child.id();
    listening_pids(mixed_port).iter().any(|p| *p == pid)
}

fn listening_pids(port: u16) -> Vec<u32> {
    let mut cmd = Command::new("netstat");
    cmd.args(["-ano", "-p", "tcp"]);
    hide_console(&mut cmd);
    let output = cmd.output().ok();
    let Some(output) = output else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let needle = format!(":{port}");
    let mut pids = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if !line.contains("LISTENING") {
            continue;
        }
        let cols: Vec<_> = line.split_whitespace().collect();
        if cols.len() < 5 {
            continue;
        }
        // Local address is usually column 1: 127.0.0.1:7890 / 0.0.0.0:7890 / [::]:7890
        let local = cols[1];
        if !local.ends_with(&needle) {
            continue;
        }
        if let Ok(pid) = cols[cols.len() - 1].parse::<u32>() {
            if pid > 0 && !pids.contains(&pid) {
                pids.push(pid);
            }
        }
    }
    pids
}

fn mihomo_pids_using_data_dir(data_key: &str) -> Vec<u32> {
    let text = process_list_mihomo();
    let mut pids = Vec::new();
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((pid_s, cmd)) = line.split_once('|') else {
            continue;
        };
        let Ok(pid) = pid_s.trim().parse::<u32>() else {
            continue;
        };
        if cmd.to_ascii_lowercase().contains(data_key) && !pids.contains(&pid) {
            pids.push(pid);
        }
    }
    pids
}

fn process_list_mihomo() -> String {
    let mut ps = Command::new("powershell");
    ps.args([
        "-NoProfile",
        "-Command",
        "Get-CimInstance Win32_Process -Filter \"Name='mihomo.exe'\" | ForEach-Object { '{0}|{1}' -f $_.ProcessId, $_.CommandLine }",
    ]);
    hide_console(&mut ps);
    if let Some(output) = ps.output().ok() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout).to_string();
            if !text.trim().is_empty() {
                return text;
            }
        }
    }

    let mut cmd = Command::new("wmic");
    cmd.args([
        "process",
        "where",
        "name='mihomo.exe'",
        "get",
        "ProcessId,CommandLine",
        "/FORMAT:LIST",
    ]);
    hide_console(&mut cmd);
    let Some(output) = cmd.output().ok() else {
        return String::new();
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut out = String::new();
    let mut cmdline = String::new();
    for raw in text.lines() {
        let line = raw.trim();
        if let Some(rest) = line.strip_prefix("CommandLine=") {
            cmdline = rest.to_string();
            continue;
        }
        if let Some(rest) = line.strip_prefix("ProcessId=") {
            if let Ok(pid) = rest.trim().parse::<u32>() {
                if !cmdline.is_empty() {
                    out.push_str(&format!("{pid}|{cmdline}\n"));
                }
            }
            cmdline.clear();
        }
    }
    out
}

fn port_accepts_local(port: u16) -> bool {
    let addr = format!("127.0.0.1:{port}");
    let Ok(sock) = addr.parse() else {
        return false;
    };
    std::net::TcpStream::connect_timeout(&sock, std::time::Duration::from_millis(250)).is_ok()
}

fn recent_core_log() -> String {
    use std::io::{Read, Seek, SeekFrom};
    let Ok(path) = paths::log_path() else {
        return String::new();
    };
    let Ok(mut file) = std::fs::File::open(path) else {
        return String::new();
    };
    let Ok(len) = file.metadata().map(|m| m.len()) else {
        return String::new();
    };
    let start = len.saturating_sub(16 * 1024);
    if file.seek(SeekFrom::Start(start)).is_err() {
        return String::new();
    }
    let mut text = String::new();
    if file.read_to_string(&mut text).is_err() {
        return String::new();
    }
    let interesting: Vec<&str> = text
        .lines()
        .rev()
        .filter(|line| {
            let lower = line.to_ascii_lowercase();
            lower.contains("level=error")
                || lower.contains("level=fatal")
                || lower.contains("listen")
                || lower.contains("already")
        })
        .take(3)
        .collect();
    interesting.into_iter().rev().collect::<Vec<_>>().join(" | ")
}

fn process_image_name(pid: u32) -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::ProcessStatus::K32GetModuleBaseNameW;
        use windows_sys::Win32::System::Threading::{
            OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
        };

        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if !handle.is_null() {
                let mut buf = [0u16; 260];
                let len = K32GetModuleBaseNameW(
                    handle,
                    std::ptr::null_mut(),
                    buf.as_mut_ptr(),
                    buf.len() as u32,
                );
                CloseHandle(handle);
                if len > 0 {
                    return Some(String::from_utf16_lossy(&buf[..len as usize]));
                }
            }
        }
        // Fallback when OpenProcess is denied (elevated foreign cores, etc.).
        let mut cmd = Command::new("tasklist");
        cmd.args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"]);
        hide_console(&mut cmd);
        let output = cmd.output().ok()?;
        let line = String::from_utf8_lossy(&output.stdout);
        let name = line.split(',').next()?.trim().trim_matches('"');
        if name.is_empty() || name.starts_with("INFO:") {
            None
        } else {
            Some(name.to_string())
        }
    }
    #[cfg(not(windows))]
    {
        let _ = pid;
        None
    }
}

fn kill_pid(pid: u32) -> bool {
    let mut cmd = Command::new("taskkill");
    cmd.args(["/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console(&mut cmd);
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

pub fn start_core(app: &AppHandle, settings: &AppSettings) -> AppResult<()> {
    let state = app.state::<CoreState>();
    if is_running(&state) {
        return Ok(());
    }

    let binary = {
        let guard = state
            .binary
            .lock()
            .map_err(|_| AppError::from("lock poisoned"))?;
        guard
            .clone()
            .ok_or_else(|| AppError::from("mihomo binary path not initialized"))?
    };

    if !binary.exists() {
        return Err(AppError::from(format!(
            "mihomo binary missing: {}",
            binary.display()
        )));
    }

    cleanup_stale_cores(settings, None);
    crate::config::regenerate(settings)?;

    let data_dir = paths::data_dir()?;
    let config = paths::config_path()?;
    let log_path = paths::log_path()?;
    let log_file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;

    let mut cmd = Command::new(&binary);
    cmd.arg("-d")
        .arg(&data_dir)
        .arg("-f")
        .arg(&config)
        .stdout(Stdio::from(log_file.try_clone()?))
        .stderr(Stdio::from(log_file))
        .current_dir(&data_dir);

    hide_console(&mut cmd);

    let child = cmd.spawn().map_err(|e| {
        AppError::from(format!(
            "启动 mihomo 失败（{}）。若已开启 TUN，请以管理员身份运行。详情：{e}",
            binary.display()
        ))
    })?;
    let child_pid = child.id();

    let mut guard = state
        .child
        .lock()
        .map_err(|_| AppError::from("lock poisoned"))?;
    *guard = Some(child);
    drop(guard);

    // GeoIP/TUN init can take well over 12s. Killing early looks like "port not listening"
    // even when the chosen port is free.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(45);
    let mut exited = false;
    loop {
        let listeners = listening_pids(settings.mixed_port);
        let ours = listeners.iter().any(|p| *p == child_pid);
        let foreign = listeners.iter().any(|p| *p != child_pid);
        let alive = is_running(&state);
        if ours || (alive && !foreign && port_accepts_local(settings.mixed_port)) {
            return Ok(());
        }
        if !alive {
            exited = true;
            break;
        }
        if std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(250));
    }

    let mixed_pids = listening_pids(settings.mixed_port);
    let _ = stop_core(app);
    let holder = mixed_pids
        .first()
        .map(|p| {
            format!(
                "PID {p} / {}",
                process_image_name(*p).unwrap_or_else(|| "unknown".into())
            )
        })
        .unwrap_or_else(|| "未监听".into());
    let extra = recent_core_log();
    let why = if exited {
        "内核进程已退出"
    } else {
        "内核仍在启动，超时内没有开始监听"
    };
    let log_hint = if extra.is_empty() {
        String::new()
    } else {
        format!(" 日志：{extra}")
    };
    Err(AppError::from(format!(
        "mixed-port {} 未能由当前内核占用（{}，现占用者：{}）。{log_hint}",
        settings.mixed_port, why, holder
    )))
}

pub fn stop_core(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<CoreState>();
    let mut guard = state
        .child
        .lock()
        .map_err(|_| AppError::from("lock poisoned"))?;
    if let Some(mut child) = guard.take() {
        let pid = child.id();
        let _ = child.kill();
        let _ = child.wait();
        let _ = kill_pid(pid);
    }
    drop(guard);
    if let Ok(settings) = AppSettings::load() {
        cleanup_stale_cores(&settings, None);
    }
    Ok(())
}

pub async fn restart_core(app: &AppHandle, settings: &AppSettings) -> AppResult<()> {
    stop_core(app)?;
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;
    start_core(app, settings)?;
    wait_until_ready(settings, 40).await?;
    let _ = restore_runtime_selections(settings).await;
    Ok(())
}

fn port_held_by_others(port: u16, our_pid: Option<u32>) -> bool {
    listening_pids(port)
        .into_iter()
        .any(|pid| Some(pid) != our_pid)
}

/// Stop, claim free ports into settings, regenerate, start.
pub async fn restart_core_with_ports(
    app: &AppHandle,
    settings: &mut AppSettings,
    allow_relocate: bool,
) -> AppResult<bool> {
    let our_pid = {
        let state = app.state::<CoreState>();
        state
            .child
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|c| c.id()))
    };

    if !allow_relocate {
        // Fail fast without stopping the core if the user-chosen ports are taken.
        if port_held_by_others(settings.mixed_port, our_pid) {
            return Err(AppError::from(format!(
                "混合端口 {} 已被其他程序占用，请关闭占用方或换一个端口",
                settings.mixed_port
            )));
        }
        let api = controller_port(settings);
        if port_held_by_others(api, our_pid) {
            return Err(AppError::from(format!(
                "控制端口 {api} 已被其他程序占用，请关闭占用方或换一个端口"
            )));
        }
        if settings.socks_port > 0 && port_held_by_others(settings.socks_port, our_pid) {
            return Err(AppError::from(format!(
                "Socks 端口 {} 已被占用",
                settings.socks_port
            )));
        }
        if settings.http_port > 0 && port_held_by_others(settings.http_port, our_pid) {
            return Err(AppError::from(format!(
                "Http 端口 {} 已被占用",
                settings.http_port
            )));
        }
    }

    stop_core(app)?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let changed = match prepare_ports(settings, allow_relocate) {
        Ok(changed) => changed,
        Err(err) => {
            // Best-effort bring the core back so the app is not left stopped.
            let _ = start_core(app, settings);
            let _ = wait_until_ready(settings, 20).await;
            return Err(err);
        }
    };
    settings.save()?;
    start_core(app, settings)?;
    wait_until_ready(settings, 40).await?;
    let _ = restore_runtime_selections(settings).await;
    Ok(changed)
}

pub async fn wait_until_ready(settings: &AppSettings, attempts: u32) -> AppResult<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/version", settings.api_base());
    for _ in 0..attempts {
        let mut req = client.get(&url);
        if !settings.secret.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", settings.secret));
        }
        if req.send().await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    Err(AppError::from("mihomo 已启动但 API 未就绪，请查看日志"))
}

pub async fn reload_config(app: &AppHandle, settings: &AppSettings) -> AppResult<()> {
    crate::config::regenerate(settings)?;
    if !is_running(&app.state::<CoreState>()) {
        start_core(app, settings)?;
        wait_until_ready(settings, 40).await?;
        let _ = restore_runtime_selections(settings).await;
        return Ok(());
    }

    let client = reqwest::Client::new();
    let path = paths::config_path()?;
    let url = format!(
        "{}/configs?force=true",
        settings.api_base().trim_end_matches('/')
    );
    let body = serde_json::json!({ "path": path.to_string_lossy() });
    let mut req = client.put(url).json(&body);
    if !settings.secret.is_empty() {
        req = req.header("Authorization", format!("Bearer {}", settings.secret));
    }
    match req.send().await {
        Ok(resp) if resp.status().is_success() => {
            let _ = restore_runtime_selections(settings).await;
            Ok(())
        }
        _ => {
            restart_core(app, settings).await?;
            let _ = restore_runtime_selections(settings).await;
            Ok(())
        }
    }
}

pub async fn restore_runtime_selections(settings: &AppSettings) -> AppResult<()> {
    use crate::settings::{POLICY_COMPANY, POLICY_DOMESTIC, POLICY_FINAL, POLICY_FOREIGN};

    if let Ok(Some(company)) = crate::company::load_enabled() {
        let selected = if settings.policy_company.trim().is_empty() {
            company.name.as_str()
        } else {
            settings.policy_company.as_str()
        };
        let _ = crate::api::select_proxy(settings, POLICY_COMPANY, selected).await;
    }
    let pairs = [
        (POLICY_DOMESTIC, settings.policy_domestic.as_str()),
        (POLICY_FOREIGN, settings.policy_foreign.as_str()),
        (POLICY_FINAL, settings.policy_final.as_str()),
    ];
    for (group, selected) in pairs {
        let _ = crate::api::select_proxy(settings, group, selected).await;
    }
    if let Some(node) = settings.selected_proxy.as_deref().filter(|s| !s.is_empty()) {
        let _ = crate::api::select_proxy(settings, "PROXY", node).await;
    }
    Ok(())
}
