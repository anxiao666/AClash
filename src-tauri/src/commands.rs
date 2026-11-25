use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::airport;
use crate::api;
use crate::company;
use crate::elevated;
use crate::error::{AppError, AppResult};
use crate::paths;
use crate::process::{self, CoreState};
use crate::settings::{normalize_rule_type, AirportProfile, AppSettings, CustomRule};
use crate::state::SettingsState;
use crate::sysproxy;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPayload {
    pub running: bool,
    pub elevated: bool,
    pub tun_enabled: bool,
    pub proxy_enabled: bool,
    pub proxy_mode: String,
    pub system_proxy: bool,
    pub mixed_port: u16,
    pub external_controller: String,
    pub mixed_port_bound: bool,
    /// WireGuard keys imported (may be disabled).
    pub company_imported: bool,
    /// Company VPN currently enabled in profile.
    pub company_enabled: bool,
    pub airport_configured: bool,
    pub airport_count: usize,
    pub selected_proxy: Option<String>,
    pub data_dir: String,
    pub version: Option<serde_json::Value>,
    pub memory_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPayload {
    pub settings: AppSettings,
    pub company: Option<company::CompanyMeta>,
    pub elevated: bool,
}

fn with_settings_mut<F, T>(state: &SettingsState, f: F) -> AppResult<T>
where
    F: FnOnce(&mut AppSettings) -> AppResult<T>,
{
    let mut guard = state
        .0
        .lock()
        .map_err(|_| AppError::from("settings lock poisoned"))?;
    let result = f(&mut guard)?;
    guard.save()?;
    Ok(result)
}

fn read_settings(state: &SettingsState) -> AppResult<AppSettings> {
    let guard = state
        .0
        .lock()
        .map_err(|_| AppError::from("settings lock poisoned"))?;
    Ok(guard.clone())
}

async fn build_status(app: &AppHandle) -> AppResult<StatusPayload> {
    let settings_state = app.state::<SettingsState>();
    let core = app.state::<CoreState>();
    let settings = read_settings(&settings_state)?;
    let running = process::is_running(&core);
    let version = if running {
        api::get_version(&settings).await.ok()
    } else {
        None
    };
    let airport_count = settings.airports.len();
    let airport_configured = settings.enabled_airports().iter().any(|a| {
        paths::airport_profile_cache_path(&a.id)
            .map(|p| p.exists())
            .unwrap_or(false)
    }) || paths::airport_cache_path()
        .map(|p| p.exists())
        .unwrap_or(false);

    Ok(StatusPayload {
        running,
        elevated: elevated::is_elevated(),
        tun_enabled: settings.tun_enabled,
        proxy_enabled: settings.proxy_enabled,
        proxy_mode: settings.proxy_mode.clone(),
        system_proxy: settings.system_proxy_active(),
        mixed_port: settings.mixed_port,
        external_controller: settings.external_controller.clone(),
        mixed_port_bound: running && process::mixed_port_owned_by_core(app, settings.mixed_port),
        company_imported: company::load_configured()?.is_some(),
        company_enabled: company::load_enabled()?.is_some(),
        airport_configured,
        airport_count,
        selected_proxy: settings.selected_proxy.clone(),
        data_dir: paths::data_dir()?.to_string_lossy().to_string(),
        version,
        memory_bytes: process::core_memory_bytes(&core),
    })
}

fn apply_takeover(settings: &AppSettings) -> AppResult<()> {
    sysproxy::set_enabled(settings.system_proxy_active(), settings.mixed_port)
}

async fn apply_proxy_change(
    app: &AppHandle,
    settings_state: &SettingsState,
    old_tun: bool,
) -> AppResult<StatusPayload> {
    let settings = read_settings(settings_state)?;
    apply_takeover(&settings)?;
    if old_tun != settings.tun_enabled {
        if settings.tun_enabled && !elevated::is_elevated() {
            // roll back tun if somehow set without elevation
            with_settings_mut(settings_state, |s| {
                s.proxy_mode = "system".into();
                s.sync_tun_flag();
                Ok(())
            })?;
            let rolled = read_settings(settings_state)?;
            apply_takeover(&rolled)?;
            crate::sync_tray_icon(app, rolled.proxy_enabled);
            return Err(AppError::from(
                "开启 TUN 需要管理员权限，请以管理员身份重新启动 AClash。",
            ));
        }
        process::restart_core(app, &read_settings(settings_state)?).await?;
    } else if !process::is_running(&app.state::<CoreState>()) {
        process::start_core(app, &settings)?;
        process::wait_until_ready(&settings, 40).await?;
    }
    crate::sync_tray_icon(app, settings.proxy_enabled);
    build_status(app).await
}

#[tauri::command]
pub async fn get_status(app: AppHandle) -> AppResult<StatusPayload> {
    build_status(&app).await
}

#[tauri::command]
pub async fn get_settings(settings_state: State<'_, SettingsState>) -> AppResult<SettingsPayload> {
    let settings = read_settings(&settings_state)?;
    let company = company::load_configured()?.map(|p| company::CompanyMeta::from(&p));
    Ok(SettingsPayload {
        settings,
        company,
        elevated: elevated::is_elevated(),
    })
}

#[tauri::command]
pub async fn save_settings(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    patch: AppSettings,
) -> AppResult<SettingsPayload> {
    let before = read_settings(&settings_state)?;
    let old_tun = before.tun_enabled;
    // Kernel page only: apply port/bind/DNS flags without clobbering
    // policy groups, nodes, airports, or custom rules from a stale UI snapshot.
    with_settings_mut(&settings_state, |settings| {
        let secret = if patch.secret.trim().is_empty() {
            settings.secret.clone()
        } else {
            patch.secret.clone()
        };
        settings.mixed_port = patch.mixed_port;
        settings.socks_port = patch.socks_port;
        settings.http_port = patch.http_port;
        settings.external_controller = patch.external_controller;
        settings.secret = secret;
        settings.allow_lan = patch.allow_lan;
        settings.ipv6 = patch.ipv6;
        settings.bind_address = patch.bind_address;
        settings.normalize_policies();
        settings.sync_tun_flag();
        if settings.tun_enabled && !elevated::is_elevated() {
            return Err(AppError::from(
                "开启 TUN 需要管理员权限，请以管理员身份重新启动 AClash。",
            ));
        }
        Ok(())
    })?;

    let mut settings = read_settings(&settings_state)?;
    let kernel_changed = before.mixed_port != settings.mixed_port
        || before.socks_port != settings.socks_port
        || before.http_port != settings.http_port
        || before.external_controller != settings.external_controller
        || before.secret != settings.secret
        || before.allow_lan != settings.allow_lan
        || before.ipv6 != settings.ipv6
        || before.bind_address != settings.bind_address;
    apply_takeover(&settings)?;
    if old_tun != settings.tun_enabled || kernel_changed {
        // User-picked ports: do not silently relocate to another free port.
        let _ = process::restart_core_with_ports(&app, &mut settings, false).await?;
        with_settings_mut(&settings_state, |s| {
            s.mixed_port = settings.mixed_port;
            s.socks_port = settings.socks_port;
            s.http_port = settings.http_port;
            s.external_controller = settings.external_controller.clone();
            Ok(())
        })?;
        let settings = read_settings(&settings_state)?;
        apply_takeover(&settings)?;
    } else {
        process::reload_config(&app, &settings).await?;
    }
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn start_core(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
) -> AppResult<StatusPayload> {
    let mut settings = read_settings(&settings_state)?;
    if settings.tun_enabled && !elevated::is_elevated() {
        return Err(AppError::from(
            "开启 TUN 需要管理员权限，请以管理员身份重新启动 AClash。",
        ));
    }
    let _ = process::prepare_ports(&mut settings, true)?;
    with_settings_mut(&settings_state, |s| {
        s.mixed_port = settings.mixed_port;
        s.external_controller = settings.external_controller.clone();
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    process::start_core(&app, &settings)?;
    process::wait_until_ready(&settings, 40).await?;
    let _ = apply_takeover(&settings);
    build_status(&app).await
}

#[tauri::command]
pub async fn stop_core(app: AppHandle) -> AppResult<StatusPayload> {
    process::stop_core(&app)?;
    build_status(&app).await
}

#[tauri::command]
pub async fn restart_core(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
) -> AppResult<StatusPayload> {
    let mut settings = read_settings(&settings_state)?;
    if settings.tun_enabled && !elevated::is_elevated() {
        return Err(AppError::from(
            "开启 TUN 需要管理员权限，请以管理员身份重新启动 AClash。",
        ));
    }
    let _ = process::restart_core_with_ports(&app, &mut settings, true).await?;
    with_settings_mut(&settings_state, |s| {
        s.mixed_port = settings.mixed_port;
        s.external_controller = settings.external_controller.clone();
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    let _ = apply_takeover(&settings);
    build_status(&app).await
}

#[tauri::command]
pub async fn set_proxy_enabled(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    enabled: bool,
) -> AppResult<StatusPayload> {
    let old_tun = read_settings(&settings_state)?.tun_enabled;
    with_settings_mut(&settings_state, |settings| {
        if enabled && settings.proxy_mode == "tun" && !elevated::is_elevated() {
            return Err(AppError::from(
                "开启 TUN 需要管理员权限，请以管理员身份重新启动 AClash。",
            ));
        }
        settings.proxy_enabled = enabled;
        settings.sync_tun_flag();
        Ok(())
    })?;
    apply_proxy_change(&app, &settings_state, old_tun).await
}

#[tauri::command]
pub async fn set_proxy_mode(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    mode: String,
) -> AppResult<StatusPayload> {
    let mode = mode.trim().to_ascii_lowercase();
    if mode != "system" && mode != "tun" {
        return Err(AppError::from("代理模式仅支持 system / tun"));
    }
    let old_tun = read_settings(&settings_state)?.tun_enabled;
    with_settings_mut(&settings_state, |settings| {
        if mode == "tun" && settings.proxy_enabled && !elevated::is_elevated() {
            return Err(AppError::from(
                "开启 TUN 需要管理员权限，请以管理员身份重新启动 AClash。",
            ));
        }
        settings.proxy_mode = mode;
        settings.sync_tun_flag();
        Ok(())
    })?;
    apply_proxy_change(&app, &settings_state, old_tun).await
}

#[tauri::command]
pub async fn set_tun(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    enabled: bool,
) -> AppResult<StatusPayload> {
    if enabled && !elevated::is_elevated() {
        return Err(AppError::from(
            "开启 TUN 需要管理员权限，请以管理员身份重新启动 AClash。",
        ));
    }
    let old_tun = read_settings(&settings_state)?.tun_enabled;
    with_settings_mut(&settings_state, |settings| {
        if enabled {
            settings.proxy_enabled = true;
            settings.proxy_mode = "tun".into();
        } else {
            settings.proxy_mode = "system".into();
        }
        settings.sync_tun_flag();
        Ok(())
    })?;
    apply_proxy_change(&app, &settings_state, old_tun).await
}

fn sync_selected_proxy(settings: &mut AppSettings, names: &[String]) {
    if settings
        .selected_proxy
        .as_ref()
        .map(|n| !names.iter().any(|x| x == n))
        .unwrap_or(true)
    {
        settings.selected_proxy = names.first().cloned();
    }
}

async fn refresh_selected_after_airports(
    app: &AppHandle,
    settings_state: &SettingsState,
) -> AppResult<()> {
    let settings = read_settings(settings_state)?;
    let proxies = airport::collect_enabled_proxies(&settings)?;
    let names = airport::proxy_names(&proxies);
    with_settings_mut(settings_state, |settings| {
        sync_selected_proxy(settings, &names);
        Ok(())
    })?;
    let settings = read_settings(settings_state)?;
    process::reload_config(app, &settings).await
}

#[tauri::command]
pub async fn add_airport_url(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    url: String,
    name: Option<String>,
) -> AppResult<SettingsPayload> {
    let url = url.trim().to_string();
    if url.is_empty() {
        return Err(AppError::from("机场订阅链接为空"));
    }
    let (yaml, info) = airport::fetch_subscription(&url).await?;
    let proxies = airport::extract_proxies(&yaml)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    airport::save_profile_cache(&id, &yaml)?;

    with_settings_mut(&settings_state, |settings| {
        let profile_name = name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| airport::guess_name_from_url(&url))
            .unwrap_or_else(|| airport::default_profile_name(settings, None));
        settings.airports.push(AirportProfile {
            id,
            name: profile_name,
            url,
            file_name: None,
            enabled: true,
            updated_at: chrono::Local::now().to_rfc3339(),
            node_count: proxies.len(),
            upload: info.upload,
            download: info.download,
            total: info.total,
            expire: info.expire,
        });
        Ok(())
    })?;

    refresh_selected_after_airports(&app, &settings_state).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn add_airport_file(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    path: String,
    name: Option<String>,
) -> AppResult<SettingsPayload> {
    let bytes = std::fs::read(&path)?;
    let yaml = airport::decode_subscription_bytes(&bytes)?;
    let proxies = airport::extract_proxies(&yaml)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    airport::save_profile_cache(&id, &yaml)?;
    let file_name = std::path::Path::new(&path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string());

    with_settings_mut(&settings_state, |settings| {
        let profile_name = name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .or_else(|| file_name.clone())
            .unwrap_or_else(|| airport::default_profile_name(settings, None));
        settings.airports.push(AirportProfile {
            id,
            name: profile_name,
            url: String::new(),
            file_name,
            enabled: true,
            updated_at: chrono::Local::now().to_rfc3339(),
            node_count: proxies.len(),
            upload: 0,
            download: 0,
            total: 0,
            expire: None,
        });
        Ok(())
    })?;

    refresh_selected_after_airports(&app, &settings_state).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn update_airport(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    id: Option<String>,
) -> AppResult<SettingsPayload> {
    let settings = read_settings(&settings_state)?;
    let targets: Vec<AirportProfile> = if let Some(id) = id {
        settings
            .airports
            .iter()
            .filter(|a| a.id == id)
            .cloned()
            .collect()
    } else {
        settings
            .airports
            .iter()
            .filter(|a| !a.url.trim().is_empty())
            .cloned()
            .collect()
    };

    if targets.is_empty() {
        return Err(AppError::from("没有可更新的机场订阅（需要带 URL 的配置）"));
    }

    for profile in targets {
        let (yaml, info) = airport::fetch_subscription(&profile.url).await?;
        let proxies = airport::extract_proxies(&yaml)?;
        airport::save_profile_cache(&profile.id, &yaml)?;
        with_settings_mut(&settings_state, |settings| {
            if let Some(item) = settings.airports.iter_mut().find(|a| a.id == profile.id) {
                item.node_count = proxies.len();
                item.updated_at = chrono::Local::now().to_rfc3339();
                item.upload = info.upload;
                item.download = info.download;
                item.total = info.total;
                item.expire = info.expire;
            }
            Ok(())
        })?;
    }

    refresh_selected_after_airports(&app, &settings_state).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn remove_airport(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    id: String,
) -> AppResult<SettingsPayload> {
    airport::remove_profile_cache(&id)?;
    with_settings_mut(&settings_state, |settings| {
        settings.airports.retain(|a| a.id != id);
        Ok(())
    })?;
    refresh_selected_after_airports(&app, &settings_state).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn set_airport_enabled(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    id: String,
    enabled: bool,
) -> AppResult<SettingsPayload> {
    with_settings_mut(&settings_state, |settings| {
        if let Some(item) = settings.airports.iter_mut().find(|a| a.id == id) {
            item.enabled = enabled;
        }
        Ok(())
    })?;
    refresh_selected_after_airports(&app, &settings_state).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn rename_airport(
    settings_state: State<'_, SettingsState>,
    id: String,
    name: String,
) -> AppResult<SettingsPayload> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::from("机场名称不能为空"));
    }
    with_settings_mut(&settings_state, |settings| {
        if let Some(item) = settings.airports.iter_mut().find(|a| a.id == id) {
            item.name = name;
        }
        Ok(())
    })?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn enable_company_from_conf(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    conf_text: String,
) -> AppResult<SettingsPayload> {
    let profile = company::enable_from_conf(&conf_text)?;
    with_settings_mut(&settings_state, |settings| {
        settings.company_imported = true;
        settings.company_domains = vec![profile.domain.clone()];
        settings.company_cidrs_override = profile.cidrs.clone();
        settings.policy_company = profile.name.clone();
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    // Hot reload — full restart_core recreates TUN and can leave WG stuck.
    process::reload_config(&app, &settings).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn set_company_enabled(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    enabled: bool,
) -> AppResult<SettingsPayload> {
    let profile = company::set_enabled(enabled)?;
    with_settings_mut(&settings_state, |settings| {
        // Keep "imported" true while keys remain on disk.
        settings.company_imported = company::has_keys(&profile);
        if enabled {
            settings.company_domains = vec![profile.domain.clone()];
            settings.company_cidrs_override = profile.cidrs.clone();
            if settings.policy_company.trim().is_empty() {
                settings.policy_company = profile.name.clone();
            }
        }
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    process::reload_config(&app, &settings).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn clear_company_wg(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
) -> AppResult<SettingsPayload> {
    company::clear_profile()?;
    with_settings_mut(&settings_state, |settings| {
        settings.company_imported = false;
        settings.policy_company.clear();
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    process::reload_config(&app, &settings).await?;
    get_settings(settings_state).await
}

#[tauri::command]
pub async fn get_company_profile() -> AppResult<company::CompanyProfile> {
    company::load_profile()
}

#[tauri::command]
pub async fn get_proxy_group(
    settings_state: State<'_, SettingsState>,
) -> AppResult<serde_json::Value> {
    let settings = read_settings(&settings_state)?;
    let proxies = api::get_proxies(&settings).await?;
    Ok(proxies
        .pointer("/proxies/PROXY")
        .cloned()
        .unwrap_or(serde_json::Value::Null))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AirportNodes {
    pub id: String,
    pub nodes: Vec<String>,
}

#[tauri::command]
pub async fn list_airport_nodes(
    settings_state: State<'_, SettingsState>,
) -> AppResult<Vec<AirportNodes>> {
    let settings = read_settings(&settings_state)?;
    Ok(airport::nodes_by_airport(&settings)?
        .into_iter()
        .map(|(id, nodes)| AirportNodes { id, nodes })
        .collect())
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleListItem {
    pub id: Option<String>,
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
    pub enabled: bool,
    pub custom: bool,
    /// custom | main | internal
    pub group: String,
    pub size: Option<i64>,
}

fn is_internal_noise_rule(rule_type: &str, payload: &str) -> bool {
    let typ = rule_type
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect::<String>();
    let payload_l = payload.trim().to_ascii_lowercase();
    if typ == "and" {
        return true;
    }
    const IP_ECHO: &[&str] = &[
        "ipify.org",
        "ip.sb",
        "ifconfig.me",
        "ipinfo.io",
        "icanhazip.com",
    ];
    if IP_ECHO
        .iter()
        .any(|h| payload_l == *h || payload_l.ends_with(&format!(".{h}")))
    {
        return true;
    }
    const PRIVATE_CIDRS: &[&str] = &[
        "0.0.0.0/8",
        "10.0.0.0/8",
        "127.0.0.0/8",
        "169.254.0.0/16",
        "172.16.0.0/12",
        "192.168.0.0/16",
        "224.0.0.0/4",
        "240.0.0.0/4",
    ];
    if (typ == "ipcidr" || typ == "ipcidr6") && PRIVATE_CIDRS.iter().any(|c| payload_l == *c) {
        return true;
    }
    false
}

fn rule_group(custom: bool, rule_type: &str, payload: &str) -> String {
    if custom {
        return "custom".into();
    }
    if is_internal_noise_rule(rule_type, payload) {
        return "internal".into();
    }
    "main".into()
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulesPayload {
    pub items: Vec<RuleListItem>,
    pub targets: Vec<String>,
}

fn rule_targets(settings: &AppSettings, company_name: Option<&str>) -> Vec<String> {
    use crate::settings::{POLICY_COMPANY, POLICY_DOMESTIC, POLICY_FINAL, POLICY_FOREIGN};
    let mut targets = vec![
        "DIRECT".into(),
        "PROXY".into(),
        POLICY_COMPANY.into(),
        POLICY_DOMESTIC.into(),
        POLICY_FOREIGN.into(),
        POLICY_FINAL.into(),
        "REJECT".into(),
    ];
    if let Some(name) = company_name {
        if !targets.iter().any(|t| t == name) {
            targets.push(name.to_string());
        }
    }
    if let Some(sel) = &settings.selected_proxy {
        if !sel.is_empty() && !targets.iter().any(|t| t == sel) {
            targets.push(sel.clone());
        }
    }
    targets
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyGroupState {
    pub name: String,
    pub now: String,
    pub all: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyGroupsPayload {
    pub company: Option<PolicyGroupState>,
    pub domestic: PolicyGroupState,
    pub foreign: PolicyGroupState,
    pub final_group: PolicyGroupState,
}

fn read_policy_group(
    proxies: &serde_json::Value,
    name: &str,
    fallback_now: &str,
    fallback_all: &[&str],
) -> PolicyGroupState {
    let path = format!("/proxies/{name}");
    let now = proxies
        .pointer(&format!("{path}/now"))
        .and_then(|v| v.as_str())
        .unwrap_or(fallback_now)
        .to_string();
    let all = proxies
        .pointer(&format!("{path}/all"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| fallback_all.iter().map(|s| (*s).to_string()).collect());
    PolicyGroupState {
        name: name.into(),
        now,
        all,
    }
}

#[tauri::command]
pub async fn get_policy_groups(
    settings_state: State<'_, SettingsState>,
) -> AppResult<PolicyGroupsPayload> {
    use crate::settings::{POLICY_COMPANY, POLICY_DOMESTIC, POLICY_FINAL, POLICY_FOREIGN};
    let settings = read_settings(&settings_state)?;
    let company = company::load_enabled()?;
    let proxies = api::get_proxies(&settings)
        .await
        .unwrap_or(serde_json::Value::Null);
    let company_state = company.as_ref().map(|c| {
        let fallback = if settings.policy_company.trim().is_empty() {
            c.name.as_str()
        } else {
            settings.policy_company.as_str()
        };
        read_policy_group(
            &proxies,
            POLICY_COMPANY,
            fallback,
            &[c.name.as_str(), "DIRECT", "PROXY"],
        )
    });
    Ok(PolicyGroupsPayload {
        company: company_state,
        domestic: read_policy_group(
            &proxies,
            POLICY_DOMESTIC,
            &settings.policy_domestic,
            &["DIRECT", "PROXY"],
        ),
        foreign: read_policy_group(
            &proxies,
            POLICY_FOREIGN,
            &settings.policy_foreign,
            &["PROXY", "DIRECT"],
        ),
        final_group: read_policy_group(
            &proxies,
            POLICY_FINAL,
            &settings.policy_final,
            &["PROXY", "DIRECT"],
        ),
    })
}

#[tauri::command]
pub async fn set_policy_group(
    settings_state: State<'_, SettingsState>,
    group: String,
    selected: String,
) -> AppResult<PolicyGroupsPayload> {
    use crate::settings::{POLICY_COMPANY, POLICY_DOMESTIC, POLICY_FINAL, POLICY_FOREIGN};
    let group = group.trim().to_string();
    let selected_raw = selected.trim().to_string();
    if selected_raw.is_empty() {
        return Err(AppError::from("出站不能为空"));
    }

    let selected = match group.as_str() {
        POLICY_COMPANY => {
            let company =
                company::load_enabled()?.ok_or_else(|| AppError::from("未启用公司 VPN"))?;
            let allowed = [company.name.as_str(), "DIRECT", "PROXY"];
            if !allowed.iter().any(|a| *a == selected_raw) {
                return Err(AppError::from("公司策略仅支持：公司节点 / DIRECT / PROXY"));
            }
            selected_raw.clone()
        }
        POLICY_DOMESTIC | POLICY_FOREIGN | POLICY_FINAL => {
            AppSettings::normalize_policy_selection(&selected_raw, "PROXY")
        }
        _ => return Err(AppError::from("策略组仅支持：公司 / 国内 / 国外 / Final")),
    };

    let settings = read_settings(&settings_state)?;
    let previous = match group.as_str() {
        POLICY_COMPANY => settings.policy_company.clone(),
        POLICY_DOMESTIC => settings.policy_domestic.clone(),
        POLICY_FOREIGN => settings.policy_foreign.clone(),
        POLICY_FINAL => settings.policy_final.clone(),
        _ => String::new(),
    };
    api::select_proxy(&settings, &group, &selected).await?;
    if let Err(e) = with_settings_mut(&settings_state, |settings| {
        match group.as_str() {
            POLICY_COMPANY => settings.policy_company = selected.clone(),
            POLICY_DOMESTIC => settings.policy_domestic = selected.clone(),
            POLICY_FOREIGN => settings.policy_foreign = selected.clone(),
            POLICY_FINAL => settings.policy_final = selected.clone(),
            _ => {}
        }
        settings.normalize_policies();
        Ok(())
    }) {
        let settings = read_settings(&settings_state)?;
        let _ = api::select_proxy(&settings, &group, &previous).await;
        return Err(e);
    }
    get_policy_groups(settings_state).await
}

#[tauri::command]
pub async fn get_rules(settings_state: State<'_, SettingsState>) -> AppResult<RulesPayload> {
    let settings = read_settings(&settings_state)?;
    let company = company::load_enabled()?;
    let company_name = company.as_ref().map(|c| c.name.as_str());
    let mut targets = rule_targets(&settings, company_name);

    // Include current PROXY members so a domain can be pinned to a node.
    if let Ok(proxies) = api::get_proxies(&settings).await {
        if let Some(all) = proxies
            .pointer("/proxies/PROXY/all")
            .and_then(|v| v.as_array())
        {
            for item in all {
                if let Some(name) = item.as_str() {
                    if name != "DIRECT" && name != "REJECT" && !targets.iter().any(|t| t == name) {
                        targets.push(name.to_string());
                    }
                }
            }
        }
    }

    let mut items: Vec<RuleListItem> = settings
        .custom_rules
        .iter()
        .map(|r| RuleListItem {
            id: Some(r.id.clone()),
            rule_type: r.rule_type.clone(),
            payload: r.payload.clone(),
            proxy: r.target.clone(),
            enabled: r.enabled,
            custom: true,
            group: "custom".into(),
            size: None,
        })
        .collect();

    if let Ok(runtime) = api::get_rules(&settings).await {
        if let Some(arr) = runtime.get("rules").and_then(|v| v.as_array()) {
            for rule in arr {
                let rule_type = rule
                    .get("type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let payload = rule
                    .get("payload")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let proxy = rule
                    .get("proxy")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let size = rule.get("size").and_then(|v| v.as_i64());
                // Skip runtime rows that duplicate an enabled custom rule.
                let dup = settings.custom_rules.iter().any(|c| {
                    c.enabled
                        && c.payload.eq_ignore_ascii_case(&payload)
                        && c.target == proxy
                        && normalize_rule_type(&c.rule_type) == normalize_rule_type(&rule_type)
                });
                if dup {
                    continue;
                }
                let group = rule_group(false, &rule_type, &payload);
                items.push(RuleListItem {
                    id: None,
                    rule_type,
                    payload,
                    proxy,
                    enabled: true,
                    custom: false,
                    group,
                    size,
                });
            }
        }
    }

    Ok(RulesPayload { items, targets })
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomRuleInput {
    pub rule_type: String,
    pub payload: String,
    pub target: String,
    pub enabled: Option<bool>,
}

#[tauri::command]
pub async fn add_custom_rule(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    input: CustomRuleInput,
) -> AppResult<RulesPayload> {
    let typ = normalize_rule_type(&input.rule_type)
        .ok_or_else(|| AppError::from("不支持的规则类型"))?
        .to_string();
    let payload = input.payload.trim().to_string();
    let target = input.target.trim().to_string();
    if payload.is_empty() {
        return Err(AppError::from("地址/匹配内容不能为空"));
    }
    if target.is_empty() {
        return Err(AppError::from("出站不能为空"));
    }
    with_settings_mut(&settings_state, |settings| {
        settings.custom_rules.push(CustomRule {
            id: uuid::Uuid::new_v4().simple().to_string(),
            rule_type: typ,
            payload,
            target,
            enabled: input.enabled.unwrap_or(true),
        });
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    process::reload_config(&app, &settings).await?;
    get_rules(settings_state).await
}

#[tauri::command]
pub async fn set_custom_rule_enabled(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    id: String,
    enabled: bool,
) -> AppResult<RulesPayload> {
    with_settings_mut(&settings_state, |settings| {
        let rule = settings
            .custom_rules
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| AppError::from("规则不存在"))?;
        rule.enabled = enabled;
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    process::reload_config(&app, &settings).await?;
    get_rules(settings_state).await
}

#[tauri::command]
pub async fn remove_custom_rule(
    app: AppHandle,
    settings_state: State<'_, SettingsState>,
    id: String,
) -> AppResult<RulesPayload> {
    with_settings_mut(&settings_state, |settings| {
        let before = settings.custom_rules.len();
        settings.custom_rules.retain(|r| r.id != id);
        if settings.custom_rules.len() == before {
            return Err(AppError::from("规则不存在"));
        }
        Ok(())
    })?;
    let settings = read_settings(&settings_state)?;
    process::reload_config(&app, &settings).await?;
    get_rules(settings_state).await
}

#[tauri::command]
pub async fn select_proxy_node(
    settings_state: State<'_, SettingsState>,
    name: String,
) -> AppResult<()> {
    let selected = name.trim().to_string();
    if selected.is_empty() {
        return Err(AppError::from("节点名称不能为空"));
    }
    let settings = read_settings(&settings_state)?;
    let previous = settings.selected_proxy.clone();

    api::select_proxy(&settings, "PROXY", &selected).await?;

    // Confirm runtime selector actually moved; stale cores / name mismatch surface here.
    let group = api::get_proxies(&settings).await?;
    let now = group
        .pointer("/proxies/PROXY/now")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if now != selected {
        if let Some(prev) = previous.as_ref() {
            let _ = api::select_proxy(&settings, "PROXY", prev).await;
        }
        return Err(AppError::from(format!(
            "节点切换未生效（当前仍为：{}）。请重启内核后重试。",
            if now.is_empty() { "未知" } else { now }
        )));
    }

    if let Err(e) = with_settings_mut(&settings_state, |settings| {
        settings.selected_proxy = Some(selected.clone());
        Ok(())
    }) {
        if let Some(prev) = previous.as_ref() {
            let _ = api::select_proxy(&settings, "PROXY", prev).await;
        }
        return Err(e);
    }

    // Drop old TCP so subsequent traffic / IP checks use the new node.
    let settings = read_settings(&settings_state)?;
    let _ = api::close_all_connections(&settings).await;
    Ok(())
}

#[tauri::command]
pub async fn get_connections(
    settings_state: State<'_, SettingsState>,
) -> AppResult<serde_json::Value> {
    let settings = read_settings(&settings_state)?;
    api::get_connections(&settings).await
}

#[tauri::command]
pub async fn close_connections(settings_state: State<'_, SettingsState>) -> AppResult<()> {
    let settings = read_settings(&settings_state)?;
    api::close_all_connections(&settings).await
}

#[tauri::command]
pub async fn read_logs(max_lines: Option<usize>) -> AppResult<String> {
    let path = paths::log_path()?;
    if !path.exists() {
        return Ok(String::new());
    }
    let content = std::fs::read_to_string(path)?;
    let max_lines = max_lines.unwrap_or(300).max(50);
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= max_lines {
        return Ok(content);
    }
    Ok(lines[lines.len() - max_lines..].join("\n"))
}

#[tauri::command]
pub async fn open_data_dir(app: AppHandle) -> AppResult<()> {
    let dir = paths::data_dir()?;
    tauri_plugin_opener::OpenerExt::opener(&app)
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::from(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn read_text_file(path: String) -> AppResult<String> {
    Ok(std::fs::read_to_string(path)?)
}

#[tauri::command]
pub async fn quit_app(app: AppHandle) -> AppResult<()> {
    let _ = sysproxy::disable();
    let _ = process::stop_core(&app);
    app.exit(0);
    Ok(())
}

#[tauri::command]
pub async fn test_proxy_delay(
    settings_state: State<'_, SettingsState>,
    name: String,
    url: Option<String>,
) -> AppResult<u32> {
    let settings = read_settings(&settings_state)?;
    api::proxy_delay(&settings, &name, 5000, url.as_deref()).await
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelayItem {
    pub name: String,
    pub delay: Option<u32>,
    pub error: Option<String>,
}

#[tauri::command]
pub async fn test_proxy_delays(
    settings_state: State<'_, SettingsState>,
    names: Vec<String>,
) -> AppResult<Vec<DelayItem>> {
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    let settings = read_settings(&settings_state)?;
    // Same defaults as Clash Party: concurrency 50, timeout 5000ms.
    let semaphore = Arc::new(Semaphore::new(50));
    let mut join_set = tokio::task::JoinSet::new();

    for name in names {
        let settings = settings.clone();
        let semaphore = semaphore.clone();
        join_set.spawn(async move {
            let _permit = match semaphore.acquire().await {
                Ok(p) => p,
                Err(_) => {
                    return DelayItem {
                        name,
                        delay: None,
                        error: Some("测速调度失败".into()),
                    };
                }
            };
            match api::proxy_delay(&settings, &name, 5000, None).await {
                Ok(delay) => DelayItem {
                    name,
                    delay: Some(delay),
                    error: None,
                },
                Err(e) => DelayItem {
                    name,
                    delay: None,
                    error: Some(e.to_string()),
                },
            }
        });
    }

    let mut out = Vec::with_capacity(join_set.len());
    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(item) => out.push(item),
            Err(e) => out.push(DelayItem {
                name: String::new(),
                delay: None,
                error: Some(e.to_string()),
            }),
        }
    }
    out.retain(|item| !item.name.is_empty());
    Ok(out)
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpItem {
    pub key: String,
    pub label: String,
    pub hint: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutboundIpPayload {
    pub items: Vec<IpItem>,
    pub via_proxy: bool,
}

#[tauri::command]
pub async fn get_outbound_ip(
    settings_state: State<'_, SettingsState>,
) -> AppResult<OutboundIpPayload> {
    let settings = read_settings(&settings_state)?;
    let via_proxy = settings.proxy_enabled;
    let mixed_port = settings.mixed_port;

    let local = api::local_ipv4().unwrap_or_else(|| "—".into());

    let mut join_set = tokio::task::JoinSet::new();
    join_set.spawn(async move {
        (
            "direct",
            api::fetch_outbound_ip(mixed_port, false)
                .await
                .unwrap_or_else(|_| "查询失败".into()),
        )
    });
    if via_proxy {
        join_set.spawn(async move {
            (
                "proxy",
                api::fetch_outbound_ip(mixed_port, true)
                    .await
                    .unwrap_or_else(|_| "查询失败".into()),
            )
        });
    }

    let mut direct_value = "查询失败".to_string();
    let mut proxy_value = if via_proxy {
        "查询失败".to_string()
    } else {
        "代理未开启".to_string()
    };
    while let Some(res) = join_set.join_next().await {
        if let Ok((key, value)) = res {
            match key {
                "direct" => direct_value = value,
                "proxy" => proxy_value = value,
                _ => {}
            }
        }
    }

    let items = vec![
        IpItem {
            key: "local".into(),
            label: "本机 IP".into(),
            hint: "局域网地址".into(),
            value: local,
        },
        IpItem {
            key: "direct".into(),
            label: "直连 IP".into(),
            hint: if settings.tun_enabled {
                "TUN 开启时探测也可能被接管，仅供参考".into()
            } else {
                "不走系统代理的公网出口".into()
            },
            value: direct_value,
        },
        IpItem {
            key: "proxy".into(),
            label: "代理出口".into(),
            hint: "经当前节点的公网 IP".into(),
            value: proxy_value,
        },
    ];

    Ok(OutboundIpPayload { items, via_proxy })
}

#[tauri::command]
pub async fn get_autostart() -> bool {
    crate::autostart::is_enabled()
}

#[tauri::command]
pub async fn set_autostart(enabled: bool) -> AppResult<bool> {
    crate::autostart::set_enabled(enabled)?;
    Ok(crate::autostart::is_enabled())
}

#[tauri::command]
pub async fn clear_logs() -> AppResult<()> {
    let path = paths::log_path()?;
    std::fs::write(path, "")?;
    Ok(())
}
