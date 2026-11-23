use serde::{Deserialize, Serialize};

/// Kind of catalog item (FR-21, DD-42). Stored as `TEXT` (`PHYSICAL` | `SERVICE`).
/// v1.1 only ever saves `Physical`; the services reject `Service` until service billing exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ItemType {
    #[default]
    Physical,
    Service,
}
