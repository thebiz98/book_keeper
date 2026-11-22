//! Compiled-in application config (`src-tauri/app.config.json`, embedded at build time).
//! Changing a value requires a rebuild; the frontend reads it via `get_app_config`.

use serde::{Deserialize, Serialize};

const COMPILED_JSON: &str = include_str!("../app.config.json");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupConfig {
    /// Hours between automatic backups while the app runs.
    pub interval_hours: u32,
    /// Automatic and pre-reset backups older than this many days are deleted.
    pub retention_days: u32,
    /// Write an automatic backup when the app closes (skipped if nothing changed).
    pub on_exit: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    /// Enables the testing-only data reset tools (UI and backend).
    pub testing_mode: bool,
    pub backup: BackupConfig,
}

impl Default for AppConfig {
    /// Safe defaults: testing tools off.
    fn default() -> Self {
        Self {
            testing_mode: false,
            backup: BackupConfig { interval_hours: 24, retention_days: 7, on_exit: true },
        }
    }
}

impl AppConfig {
    /// The config embedded in this build. Panics only if the committed JSON is invalid,
    /// which the unit test below catches at `cargo test` time.
    pub fn compiled() -> Self {
        Self::parse(COMPILED_JSON).expect("src-tauri/app.config.json is invalid")
    }

    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn committed_config_parses() {
        let cfg = AppConfig::compiled();
        assert!(cfg.backup.interval_hours > 0);
        assert!(cfg.backup.retention_days > 0);
    }

    #[test]
    fn parse_reads_camel_case_fields() {
        let cfg = AppConfig::parse(
            r#"{"testingMode":false,"backup":{"intervalHours":6,"retentionDays":3,"onExit":false}}"#,
        )
        .unwrap();
        assert!(!cfg.testing_mode);
        assert_eq!(cfg.backup, BackupConfig { interval_hours: 6, retention_days: 3, on_exit: false });
    }
}
