use std::sync::Mutex;

use crate::settings::AppSettings;

pub struct SettingsState(pub Mutex<AppSettings>);
