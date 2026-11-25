use std::path::PathBuf;

use crate::error::{AppError, AppResult};

pub const APP_DIR_NAME: &str = "AClash";
const LEGACY_APP_DIR_NAME: &str = "mihomo-lite";

pub fn data_dir() -> AppResult<PathBuf> {
    let base =
        dirs::data_dir().ok_or_else(|| AppError::from("cannot resolve app data directory"))?;
    let dir = base.join(APP_DIR_NAME);
    let legacy = base.join(LEGACY_APP_DIR_NAME);
    if !dir.exists() && legacy.exists() {
        // Prefer rename so airport/company settings carry over after rebrand.
        if std::fs::rename(&legacy, &dir).is_err() {
            let _ = copy_dir_recursive(&legacy, &dir);
        }
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::create_dir_all(dir.join("airport"))?;
    std::fs::create_dir_all(dir.join("company"))?;
    std::fs::create_dir_all(dir.join("logs"))?;
    Ok(dir)
}

fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

pub fn settings_path() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("app.json"))
}

pub fn config_path() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("config.yaml"))
}

pub fn airport_cache_path() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("airport").join("subscription.yaml"))
}

pub fn airport_profile_cache_path(id: &str) -> AppResult<PathBuf> {
    Ok(data_dir()?.join("airport").join(format!("{id}.yaml")))
}

pub fn company_conf_path() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("company").join("wireguard.conf"))
}

pub fn company_meta_path() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("company").join("meta.json"))
}

pub fn log_path() -> AppResult<PathBuf> {
    Ok(data_dir()?.join("logs").join("mihomo.log"))
}

pub fn resolve_mihomo_binary(resource_dir: &std::path::Path) -> AppResult<PathBuf> {
    let candidates = [
        resource_dir.join("resources").join("mihomo.exe"),
        resource_dir.join("mihomo.exe"),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("mihomo.exe")))
            .unwrap_or_default(),
    ];

    for path in candidates {
        if path.is_file() {
            return Ok(path);
        }
    }

    Err(AppError::from(
        "未找到 mihomo.exe，请放到 src-tauri/resources/mihomo.exe",
    ))
}
