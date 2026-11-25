use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::paths;

/// Company routing template: `company/template.json` under the data dir.
/// Domains and networks stay out of the source tree; keys come from the imported conf.
pub fn default_profile() -> CompanyProfile {
    let template = paths::data_dir()
        .ok()
        .map(|dir| dir.join("company").join("template.json"))
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|raw| serde_json::from_str::<CompanyProfile>(&raw).ok());
    if let Some(mut profile) = template {
        profile.enabled = false;
        profile.private_key = String::new();
        profile.public_key = String::new();
        profile.pre_shared_key = None;
        profile.updated_at = String::new();
        return profile;
    }
    CompanyProfile {
        enabled: false,
        private_key: String::new(),
        public_key: String::new(),
        pre_shared_key: None,
        name: "公司VPN".into(),
        domain: String::new(),
        server: String::new(),
        port: 0,
        client_ip: String::new(),
        dns: String::new(),
        allowed_ips: Vec::new(),
        mtu: None,
        persistent_keepalive: None,
        cidrs: Vec::new(),
        conflict_ips: Vec::new(),
        custom_company: Vec::new(),
        custom_direct: Vec::new(),
        custom_proxy: Vec::new(),
        updated_at: String::new(),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyProfile {
    pub enabled: bool,
    pub private_key: String,
    pub public_key: String,
    pub pre_shared_key: Option<String>,
    pub name: String,
    pub domain: String,
    pub server: String,
    pub port: u16,
    pub client_ip: String,
    pub dns: String,
    #[serde(default)]
    pub allowed_ips: Vec<String>,
    #[serde(default)]
    pub mtu: Option<u16>,
    #[serde(default)]
    pub persistent_keepalive: Option<u16>,
    pub cidrs: Vec<String>,
    pub conflict_ips: Vec<String>,
    pub custom_company: Vec<String>,
    pub custom_direct: Vec<String>,
    pub custom_proxy: Vec<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompanyMeta {
    pub enabled: bool,
    pub domains: Vec<String>,
    pub cidrs: Vec<String>,
    pub dns_servers: Vec<String>,
    pub endpoint: String,
    pub imported_at: String,
    pub conflict_ips: Vec<String>,
    pub name: String,
    pub domain: String,
    pub allowed_ips: Vec<String>,
    pub mtu: Option<u16>,
    pub persistent_keepalive: Option<u16>,
}

impl From<&CompanyProfile> for CompanyMeta {
    fn from(p: &CompanyProfile) -> Self {
        let mut domains = vec![p.domain.clone()];
        domains.extend(p.custom_company.clone());
        Self {
            enabled: p.enabled && !p.private_key.is_empty() && !p.public_key.is_empty(),
            domains,
            cidrs: p.cidrs.clone(),
            dns_servers: vec![p.dns.clone()],
            endpoint: format!("{}:{}", p.server, p.port),
            imported_at: p.updated_at.clone(),
            conflict_ips: p.conflict_ips.clone(),
            name: p.name.clone(),
            domain: p.domain.clone(),
            allowed_ips: p.allowed_ips.clone(),
            mtu: p.mtu,
            persistent_keepalive: p.persistent_keepalive,
        }
    }
}

#[derive(Debug, Default)]
struct WgKeys {
    private_key: Option<String>,
    public_key: Option<String>,
    pre_shared_key: Option<String>,
    address: Option<String>,
    endpoint: Option<String>,
    dns: Option<String>,
    allowed_ips: Vec<String>,
    mtu: Option<u16>,
    persistent_keepalive: Option<u16>,
}

pub fn profile_path() -> AppResult<std::path::PathBuf> {
    Ok(paths::data_dir()?.join("company").join("profile.json"))
}

pub fn load_profile() -> AppResult<CompanyProfile> {
    let path = profile_path()?;
    if !path.exists() {
        return Ok(default_profile());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&raw)?)
}

pub fn save_profile(profile: &CompanyProfile) -> AppResult<()> {
    let path = profile_path()?;
    std::fs::write(path, serde_json::to_string_pretty(profile)?)?;
    Ok(())
}

pub fn clear_profile() -> AppResult<()> {
    let path = profile_path()?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    // legacy files
    let conf = paths::company_conf_path()?;
    let meta = paths::company_meta_path()?;
    if conf.exists() {
        let _ = std::fs::remove_file(conf);
    }
    if meta.exists() {
        let _ = std::fs::remove_file(meta);
    }
    Ok(())
}

/// Enable company VPN from a WireGuard .conf.
/// WireGuard transport fields from conf override the built-in template;
/// domain / CIDRs / conflict IPs stay on the company template.
pub fn enable_from_conf(raw: &str) -> AppResult<CompanyProfile> {
    let keys = parse_keys(raw)?;
    enable_from_keys(
        keys.private_key
            .ok_or_else(|| AppError::from("配置缺少 PrivateKey"))?,
        keys.public_key
            .ok_or_else(|| AppError::from("配置缺少 PublicKey"))?,
        keys.pre_shared_key,
        keys.address,
        keys.endpoint,
        keys.dns,
        keys.allowed_ips,
        keys.mtu,
        keys.persistent_keepalive,
    )
}

pub fn enable_from_keys(
    private_key: String,
    public_key: String,
    pre_shared_key: Option<String>,
    client_ip: Option<String>,
    endpoint: Option<String>,
    dns: Option<String>,
    allowed_ips: Vec<String>,
    mtu: Option<u16>,
    persistent_keepalive: Option<u16>,
) -> AppResult<CompanyProfile> {
    let mut profile = load_profile().unwrap_or_else(|_| default_profile());
    if profile.domain.trim().is_empty() {
        return Err(AppError::from(
            "缺少公司域名模板，请在数据目录 company/template.json 中配置 domain",
        ));
    }
    profile.private_key = private_key.trim().to_string();
    profile.public_key = public_key.trim().to_string();
    profile.pre_shared_key = pre_shared_key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(ip) = client_ip {
        let ip = ip.split('/').next().unwrap_or(&ip).trim().to_string();
        if !ip.is_empty() && !ip.contains(':') {
            profile.client_ip = ip;
        }
    }
    if let Some(ep) = endpoint.as_deref().and_then(parse_endpoint) {
        profile.server = ep.0;
        profile.port = ep.1;
    }
    if let Some(dns) = dns {
        let dns = dns.split(',').next().unwrap_or(&dns).trim().to_string();
        if !dns.is_empty() {
            profile.dns = dns;
        }
    }
    profile.allowed_ips = allowed_ips;
    profile.mtu = mtu;
    profile.persistent_keepalive = persistent_keepalive;
    if profile.server.trim().is_empty() {
        profile.server = format!("navigator.{}", profile.domain);
    }
    profile.enabled = true;
    profile.updated_at = chrono::Local::now().to_rfc3339();
    if profile.private_key.is_empty() || profile.public_key.is_empty() {
        return Err(AppError::from("PrivateKey / PublicKey 不能为空"));
    }
    save_profile(&profile)?;
    Ok(profile)
}

/// Parse `host:port` or `[ipv6]:port`.
fn parse_endpoint(raw: &str) -> Option<(String, u16)> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(rest) = raw.strip_prefix('[') {
        let (host, port_part) = rest.split_once("]:")?;
        let port: u16 = port_part.trim().parse().ok()?;
        if host.is_empty() || port == 0 {
            return None;
        }
        return Some((host.to_string(), port));
    }
    let (host, port_s) = raw.rsplit_once(':')?;
    let port: u16 = port_s.parse().ok()?;
    if host.is_empty() || port == 0 {
        return None;
    }
    Some((host.to_string(), port))
}

pub fn has_keys(profile: &CompanyProfile) -> bool {
    !profile.private_key.trim().is_empty() && !profile.public_key.trim().is_empty()
}

/// Profile with WireGuard keys saved (may be disabled).
pub fn load_configured() -> AppResult<Option<CompanyProfile>> {
    let profile = load_profile()?;
    if has_keys(&profile) {
        Ok(Some(profile))
    } else {
        Ok(None)
    }
}

pub fn load_enabled() -> AppResult<Option<CompanyProfile>> {
    let profile = load_profile()?;
    if profile.enabled && has_keys(&profile) {
        Ok(Some(profile))
    } else {
        Ok(None)
    }
}

/// Toggle company VPN without deleting keys. Enable requires imported conf.
pub fn set_enabled(enabled: bool) -> AppResult<CompanyProfile> {
    let mut profile = load_profile()?;
    if enabled && !has_keys(&profile) {
        return Err(AppError::from("请先导入 WireGuard 配置文件"));
    }
    profile.enabled = enabled;
    profile.updated_at = chrono::Local::now().to_rfc3339();
    save_profile(&profile)?;
    Ok(profile)
}

fn parse_keys(raw: &str) -> AppResult<WgKeys> {
    let mut draft = WgKeys::default();
    let mut section = String::new();
    for line in raw.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_ascii_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match (section.as_str(), key.as_str()) {
            ("interface", "privatekey") => draft.private_key = Some(value.to_string()),
            ("interface", "address") => {
                draft.address = value.split(',').next().map(|s| s.trim().to_string());
            }
            ("interface", "dns") => {
                draft.dns = value.split(',').next().map(|s| s.trim().to_string());
            }
            ("interface", "mtu") => {
                draft.mtu = value.parse::<u16>().ok().filter(|v| *v >= 576);
            }
            ("peer", "publickey") => draft.public_key = Some(value.to_string()),
            ("peer", "presharedkey") => draft.pre_shared_key = Some(value.to_string()),
            ("peer", "endpoint") => draft.endpoint = Some(value.to_string()),
            ("peer", "allowedips") => {
                draft.allowed_ips.extend(
                    value
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string),
                );
            }
            ("peer", "persistentkeepalive") => {
                draft.persistent_keepalive = value.parse::<u16>().ok().filter(|v| *v > 0);
            }
            _ => {}
        }
    }
    Ok(draft)
}

/// Junk node names to hide from PROXY select (airport info nodes).
pub fn exclude_node_regex(company_name: &str) -> String {
    format!(
        "^{}$|剩余|到期|过期|重置|流量|官网|套餐|客服|公告|到期时间|距离下次重置",
        regex_escape(company_name)
    )
}

fn regex_escape(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '.' | '+' | '*' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\' => {
                format!("\\{c}")
            }
            _ => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::parse_keys;

    #[test]
    fn parses_wireguard_transport_options() {
        let conf = r#"
[Interface]
PrivateKey = private
Address = 10.20.0.2/32
DNS = 10.20.0.1
MTU = 1360

[Peer]
PublicKey = public
PresharedKey = shared
AllowedIPs = 10.30.0.0/24, 10.40.0.0/24
Endpoint = vpn.example.com:1195
PersistentKeepalive = 20
"#;
        let parsed = parse_keys(conf).expect("valid conf");

        assert_eq!(parsed.mtu, Some(1360));
        assert_eq!(parsed.persistent_keepalive, Some(20));
        assert_eq!(parsed.allowed_ips, vec!["10.30.0.0/24", "10.40.0.0/24"]);
        assert_eq!(parsed.endpoint.as_deref(), Some("vpn.example.com:1195"));
    }
}
