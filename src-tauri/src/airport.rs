use base64::Engine;
use serde_yaml::Value;
use std::collections::HashSet;

use crate::error::{AppError, AppResult};
use crate::paths;
use crate::settings::AppSettings;

#[derive(Debug, Default, Clone)]
pub struct SubUserInfo {
    pub upload: u64,
    pub download: u64,
    pub total: u64,
    pub expire: Option<i64>,
}

pub async fn fetch_subscription(url: &str) -> AppResult<(String, SubUserInfo)> {
    let client = reqwest::Client::builder()
        .user_agent("AClash/0.1")
        .gzip(true)
        .build()?;
    let response = client.get(url).send().await?.error_for_status()?;
    let info = parse_userinfo_headers(response.headers());
    let bytes = response.bytes().await?;
    let yaml = decode_subscription_bytes(&bytes)?;
    Ok((yaml, info))
}

fn parse_userinfo_headers(headers: &reqwest::header::HeaderMap) -> SubUserInfo {
    let mut info = SubUserInfo::default();
    let raw = headers
        .get("subscription-userinfo")
        .or_else(|| headers.get("Subscription-Userinfo"))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    for part in raw.split(';') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            match k.trim().to_ascii_lowercase().as_str() {
                "upload" => info.upload = v.trim().parse().unwrap_or(0),
                "download" => info.download = v.trim().parse().unwrap_or(0),
                "total" => info.total = v.trim().parse().unwrap_or(0),
                "expire" => info.expire = v.trim().parse().ok(),
                _ => {}
            }
        }
    }
    info
}

pub fn decode_subscription_bytes(bytes: &[u8]) -> AppResult<String> {
    let text = String::from_utf8_lossy(bytes).to_string();
    let trimmed = text.trim();
    if looks_like_yaml(trimmed) {
        return Ok(trimmed.to_string());
    }

    let cleaned = trimmed
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>();
    if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(&cleaned) {
        let decoded_text = String::from_utf8_lossy(&decoded).to_string();
        if looks_like_yaml(decoded_text.trim()) {
            return Ok(decoded_text);
        }
    }

    if looks_like_yaml(trimmed) {
        return Ok(trimmed.to_string());
    }

    Err(AppError::from(
        "订阅内容不是 Clash/Mihomo YAML（明文或 Base64）",
    ))
}

pub fn extract_proxies(yaml_text: &str) -> AppResult<Vec<Value>> {
    let root: Value = serde_yaml::from_str(yaml_text)?;
    let proxies = root
        .get("proxies")
        .and_then(|v| v.as_sequence())
        .ok_or_else(|| AppError::from("机场配置缺少 proxies 列表"))?;

    if proxies.is_empty() {
        return Err(AppError::from("机场配置 proxies 列表为空"));
    }

    let mut out = Vec::new();
    for proxy in proxies {
        if proxy.get("name").and_then(|v| v.as_str()).is_some() {
            out.push(proxy.clone());
        }
    }
    if out.is_empty() {
        return Err(AppError::from("机场配置中没有有效节点"));
    }
    Ok(out)
}

pub fn proxy_names(proxies: &[Value]) -> Vec<String> {
    proxies
        .iter()
        .filter_map(|p| {
            p.get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect()
}

pub fn save_profile_cache(id: &str, yaml_text: &str) -> AppResult<()> {
    std::fs::write(paths::airport_profile_cache_path(id)?, yaml_text)?;
    Ok(())
}

pub fn load_profile_cache(id: &str) -> AppResult<Option<String>> {
    let path = paths::airport_profile_cache_path(id)?;
    if !path.exists() {
        return Ok(None);
    }
    Ok(Some(std::fs::read_to_string(path)?))
}

pub fn remove_profile_cache(id: &str) -> AppResult<()> {
    let path = paths::airport_profile_cache_path(id)?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

pub fn collect_enabled_proxies(settings: &AppSettings) -> AppResult<Vec<Value>> {
    let mut merged = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();

    for profile in settings.enabled_airports() {
        let Some(yaml) = load_profile_cache(&profile.id)? else {
            continue;
        };
        let proxies = extract_proxies(&yaml)?;
        for proxy in proxies {
            let renamed = uniquify_proxy(proxy, &profile.name, &mut used_names);
            merged.push(renamed);
        }
    }

    if merged.is_empty() {
        let legacy = paths::airport_cache_path()?;
        if legacy.exists() {
            let yaml = std::fs::read_to_string(legacy)?;
            let proxies = extract_proxies(&yaml)?;
            for proxy in proxies {
                let renamed = uniquify_proxy(proxy, "机场", &mut used_names);
                merged.push(renamed);
            }
        }
    }

    Ok(merged)
}

/// Node names per airport, matching PROXY select names for enabled profiles.
pub fn nodes_by_airport(settings: &AppSettings) -> AppResult<Vec<(String, Vec<String>)>> {
    let mut out = Vec::new();
    let mut used_names: HashSet<String> = HashSet::new();

    for profile in &settings.airports {
        let mut names = Vec::new();
        if let Some(yaml) = load_profile_cache(&profile.id)? {
            if let Ok(proxies) = extract_proxies(&yaml) {
                for proxy in proxies {
                    let renamed = uniquify_proxy(proxy, &profile.name, &mut used_names);
                    if let Some(n) = renamed.get("name").and_then(|v| v.as_str()) {
                        names.push(n.to_string());
                    }
                }
            }
        }
        out.push((profile.id.clone(), names));
    }

    Ok(out)
}

pub fn default_profile_name(settings: &AppSettings, hint: Option<&str>) -> String {
    if let Some(hint) = hint.map(str::trim).filter(|s| !s.is_empty()) {
        return hint.to_string();
    }
    format!("机场 {}", settings.airports.len() + 1)
}

pub fn guess_name_from_url(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;
    parsed.host_str().map(|h| h.to_string())
}

fn uniquify_proxy(mut proxy: Value, airport_name: &str, used: &mut HashSet<String>) -> Value {
    let original = proxy
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("node")
        .to_string();
    let mut name = original.clone();
    if used.contains(&name) {
        name = format!("[{airport_name}] {original}");
    }
    let mut i = 2;
    while used.contains(&name) {
        name = format!("[{airport_name}] {original} #{i}");
        i += 1;
    }
    used.insert(name.clone());
    if let Value::Mapping(map) = &mut proxy {
        map.insert(Value::from("name"), Value::from(name));
    }
    proxy
}

fn looks_like_yaml(text: &str) -> bool {
    text.contains("proxies:") || text.contains("proxy-groups:") || text.starts_with("port:")
}
