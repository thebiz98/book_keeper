use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type", content = "details")]
pub enum AppError {
    #[error("Validation Error: {message}")]
    ValidationError { field: String, message: String },

    #[error("Insufficient stock for item '{item_name}'. Required: {required}, Available: {available}")]
    InsufficientStockError {
        item_id: i64,
        item_name: String,
        required: f64,
        available: f64,
    },

    #[error("Entity not found: {entity} with identifier {id}")]
    NotFound { entity: String, id: String },

    #[error("Database concurrency conflict: {0}")]
    ConcurrencyConflict(String),

    #[error("Database execution error: {0}")]
    DatabaseError(String),

    #[error("Search index error: {0}")]
    SearchIndexError(String),

    #[error("Migration error: {0}")]
    MigrationError(String),

    #[error("IO / File system error: {0}")]
    IoError(String),
}

pub type DomainResult<T> = Result<T, AppError>;

impl AppError {
    pub fn not_found(entity: impl Into<String>, id: impl Into<String>) -> Self {
        Self::NotFound {
            entity: entity.into(),
            id: id.into(),
        }
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(err: rusqlite::Error) -> Self {
        AppError::DatabaseError(err.to_string())
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        AppError::IoError(err.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(err: serde_json::Error) -> Self {
        AppError::ValidationError {
            field: "json".to_string(),
            message: err.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_deserialization_not_found() {
        let dynamic_json = format!(r#"{{"type":"NotFound","details":{{"entity":"Item","id":"{}"}}}}"#, 999);
        let err: AppError = serde_json::from_str(&dynamic_json).expect("should deserialize from dynamic string");
        match err {
            AppError::NotFound { entity, id } => {
                assert_eq!(entity, "Item");
                assert_eq!(id, "999");
            }
            _ => panic!("wrong variant"),
        }
    }
}
