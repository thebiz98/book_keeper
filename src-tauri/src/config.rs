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
    /// IANA zone name of the shop (e.g. `Asia/Kolkata`): drives "today", backup stamps and shown
    /// times. Stored data stays UTC, so changing it needs no data change.
    #[serde(default = "default_tz")]
    pub time_zone: String,
    pub backup: BackupConfig,
}

fn default_tz() -> String {
    "Asia/Kolkata".to_string()
}

impl Default for AppConfig {
    /// Safe defaults: testing tools off.
    fn default() -> Self {
        Self {
            testing_mode: false,
            time_zone: default_tz(),
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

    /// Parses the JSON; an unknown time zone name is an error.
    pub fn parse(json: &str) -> Result<Self, serde_json::Error> {
        let cfg: Self = serde_json::from_str(json)?;
        cfg.time_zone
            .parse::<chrono_tz::Tz>()
            .map_err(|e| <serde_json::Error as serde::de::Error>::custom(format!("timeZone '{}': {e}", cfg.time_zone)))?;
        Ok(cfg)
    }

    /// The configured shop time zone (validated by `parse`; falls back to Asia/Kolkata).
    pub fn tz(&self) -> chrono_tz::Tz {
        self.time_zone.parse().unwrap_or(chrono_tz::Asia::Kolkata)
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
        assert!(cfg.time_zone.parse::<chrono_tz::Tz>().is_ok());
    }

    #[test]
    fn missing_time_zone_defaults_to_kolkata() {
        let cfg = AppConfig::parse(r#"{"testingMode":false,"backup":{"intervalHours":6,"retentionDays":3,"onExit":false}}"#)
            .unwrap();
        assert_eq!(cfg.time_zone, "Asia/Kolkata");
        assert_eq!(cfg.tz(), chrono_tz::Asia::Kolkata);
    }

    #[test]
    fn unknown_time_zone_is_rejected() {
        let r = AppConfig::parse(
            r#"{"testingMode":false,"timeZone":"Mars/Base","backup":{"intervalHours":6,"retentionDays":3,"onExit":false}}"#,
        );
        assert!(r.is_err());
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
