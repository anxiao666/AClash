use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::paths;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirportProfile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub node_count: usize,
    #[serde(default)]
    pub upload: u64,
    #[serde(default)]
    pub download: u64,
    #[serde(default)]
    pub total: u64,
    #[serde(default)]
    pub expire: Option<i64>,
}

fn default_true() -> bool {
    true
}

fn default_proxy_mode() -> String {
    "system".into()
}

fn default_policy_domestic() -> String {
    "DIRECT".into()
}

fn default_policy_foreign() -> String {
    "PROXY".into()
}

fn default_policy_final() -> String {
    "PROXY".into()
}

pub const POLICY_COMPANY: &str = "公司";
pub const POLICY_DOMESTIC: &str = "国内";
pub const POLICY_FOREIGN: &str = "国外";
pub const POLICY_FINAL: &str = "Final";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomRule {
    pub id: String,
    /// DOMAIN | DOMAIN-SUFFIX | IP-CIDR | IP-CIDR6
    pub rule_type: String,
    pub payload: String,
    /// DIRECT | PROXY | REJECT | company / node name
    pub target: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub airports: Vec<AirportProfile>,
    /// Legacy single-airport fields kept for migration.
    #[serde(default)]
    pub airport_url: String,
    #[serde(default)]
    pub airport_file_name: Option<String>,
    pub mixed_port: u16,
    /// 0 = disabled
    #[serde(default)]
    pub socks_port: u16,
    /// 0 = disabled (Clash `port` / HTTP proxy)
    #[serde(default)]
    pub http_port: u16,
    pub external_controller: String,
    pub secret: String,
    #[serde(default)]
    pub allow_lan: bool,
    #[serde(default)]
    pub ipv6: bool,
    /// Empty = mihomo default bind
    #[serde(default)]
    pub bind_address: String,
    /// Derived: true when proxy is enabled and mode is tun.
    #[serde(default)]
    pub tun_enabled: bool,
    /// Master switch for traffic takeover (system proxy / TUN).
    #[serde(default)]
    pub proxy_enabled: bool,
    /// "system" | "tun"
    #[serde(default = "default_proxy_mode")]
    pub proxy_mode: String,
    pub lan_direct: bool,
    pub company_domains: Vec<String>,
    pub company_cidrs_override: Vec<String>,
    pub company_imported: bool,
    pub selected_proxy: Option<String>,
    /// Selected outbound for 国内 policy group: DIRECT | PROXY
    #[serde(default = "default_policy_domestic")]
    pub policy_domestic: String,
    /// Selected outbound for 国外 policy group: PROXY | DIRECT
    #[serde(default = "default_policy_foreign")]
    pub policy_foreign: String,
    /// Selected outbound for Final policy group: PROXY | DIRECT
    #[serde(default = "default_policy_final")]
    pub policy_final: String,
    /// Selected outbound for 公司 policy group: company node | DIRECT | PROXY
    #[serde(default)]
    pub policy_company: String,
    #[serde(default)]
    pub custom_rules: Vec<CustomRule>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            airports: Vec::new(),
            airport_url: String::new(),
            airport_file_name: None,
            mixed_port: 7890,
            socks_port: 0,
            http_port: 0,
            external_controller: "127.0.0.1:9090".into(),
            secret: uuid::Uuid::new_v4().simple().to_string(),
            allow_lan: false,
            ipv6: false,
            bind_address: String::new(),
            tun_enabled: false,
            proxy_enabled: false,
            proxy_mode: default_proxy_mode(),
            lan_direct: true,
            company_domains: Vec::new(),
            company_cidrs_override: Vec::new(),
            company_imported: false,
            selected_proxy: None,
            policy_domestic: default_policy_domestic(),
            policy_foreign: default_policy_foreign(),
            policy_final: default_policy_final(),
            policy_company: String::new(),
            custom_rules: Vec::new(),
        }
    }
}

impl CustomRule {
    pub fn to_clash_line(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        let payload = self.payload.trim();
        let target = self.target.trim();
        if payload.is_empty() || target.is_empty() {
            return None;
        }
        let typ = normalize_rule_type(&self.rule_type)?;
        let line = if typ == "IP-CIDR" || typ == "IP-CIDR6" {
            format!("{typ},{payload},{target},no-resolve")
        } else {
            format!("{typ},{payload},{target}")
        };
        Some(line)
    }
}

pub fn normalize_rule_type(raw: &str) -> Option<&'static str> {
    let key: String = raw
        .trim()
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    match key.as_str() {
        "domain" => Some("DOMAIN"),
        "domainsuffix" => Some("DOMAIN-SUFFIX"),
        "domainkeyword" => Some("DOMAIN-KEYWORD"),
        "ipcidr" => Some("IP-CIDR"),
        "ipcidr6" => Some("IP-CIDR6"),
        _ => None,
    }
}

impl AppSettings {
    pub fn load() -> AppResult<Self> {
        let path = paths::settings_path()?;
        if !path.exists() {
            let settings = Self::default();
            settings.save()?;
            return Ok(settings);
        }
        let raw = std::fs::read_to_string(path)?;
        let mut settings: Self = serde_json::from_str(&raw)?;
        settings.migrate_legacy_airport()?;
        settings.migrate_proxy_fields();
        settings.sync_tun_flag();
        settings.normalize_policies();
        for airport in &mut settings.airports {
            airport.enabled = true;
        }
        Ok(settings)
    }

    pub fn save(&self) -> AppResult<()> {
        let path = paths::settings_path()?;
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(path, raw)?;
        Ok(())
    }

    pub fn api_base(&self) -> String {
        format!("http://{}", self.external_controller.trim())
    }

    pub fn enabled_airports(&self) -> Vec<&AirportProfile> {
        // Subscriptions are always active; keep/remove instead of enable/disable.
        self.airports.iter().collect()
    }

    pub fn normalize_policy_selection(raw: &str, fallback: &str) -> String {
        let v = raw.trim();
        if v.eq_ignore_ascii_case("DIRECT") {
            "DIRECT".into()
        } else if v.eq_ignore_ascii_case("PROXY") {
            "PROXY".into()
        } else {
            fallback.into()
        }
    }

    pub fn normalize_policies(&mut self) {
        self.policy_domestic =
            Self::normalize_policy_selection(&self.policy_domestic, &default_policy_domestic());
        self.policy_foreign =
            Self::normalize_policy_selection(&self.policy_foreign, &default_policy_foreign());
        self.policy_final =
            Self::normalize_policy_selection(&self.policy_final, &default_policy_final());
        self.policy_company = self.policy_company.trim().to_string();
    }

    pub fn sync_tun_flag(&mut self) {
        let mode = self.proxy_mode.trim().to_ascii_lowercase();
        if mode != "tun" && mode != "system" {
            self.proxy_mode = default_proxy_mode();
        }
        self.tun_enabled = self.proxy_enabled && self.proxy_mode == "tun";
    }

    pub fn system_proxy_active(&self) -> bool {
        self.proxy_enabled && self.proxy_mode == "system"
    }

    fn migrate_proxy_fields(&mut self) {
        // Old installs only had tun_enabled.
        if self.tun_enabled && !self.proxy_enabled {
            self.proxy_enabled = true;
            self.proxy_mode = "tun".into();
        }
    }

    fn migrate_legacy_airport(&mut self) -> AppResult<()> {
        if !self.airports.is_empty() {
            return Ok(());
        }
        let legacy_cache = paths::airport_cache_path()?;
        let has_legacy = !self.airport_url.trim().is_empty()
            || self.airport_file_name.is_some()
            || legacy_cache.exists();
        if !has_legacy {
            return Ok(());
        }

        let id = uuid::Uuid::new_v4().simple().to_string();
        let name = self
            .airport_file_name
            .clone()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| {
                if self.airport_url.trim().is_empty() {
                    "默认机场".into()
                } else {
                    "机场 1".into()
                }
            });

        if legacy_cache.exists() {
            let dest = paths::airport_profile_cache_path(&id)?;
            std::fs::copy(&legacy_cache, dest)?;
        }

        let node_count = if legacy_cache.exists() {
            let yaml = std::fs::read_to_string(&legacy_cache).unwrap_or_default();
            crate::airport::extract_proxies(&yaml)
                .map(|p| p.len())
                .unwrap_or(0)
        } else {
            0
        };

        self.airports.push(AirportProfile {
            id,
            name,
            url: self.airport_url.clone(),
            file_name: self.airport_file_name.clone(),
            enabled: true,
            updated_at: chrono::Local::now().to_rfc3339(),
            node_count,
            upload: 0,
            download: 0,
            total: 0,
            expire: None,
        });
        self.save()?;
        Ok(())
    }
}
