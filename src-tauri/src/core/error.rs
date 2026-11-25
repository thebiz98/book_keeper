use serde::{Deserialize, Serialize};

/// Typed error returned by every IPC command (DD-26).
///
/// Wire shape is exactly `{ "type": <variant>, "details": <payload> }`:
/// tuple variants carry `details` as a plain string (already a human message);
/// struct variants carry `details` as an object of their named fields.
///
/// Message decision: the variants are NOT given a `details.message` field.
/// Instead the frontend derives the message (`errorMessage` in `src/api/errors.ts`):
/// `ValidationError` -> `details.message`; string `details` -> itself;
/// `InsufficientStockError` / `NotFound` -> built from their fields, mirroring `Display`.
/// Field names inside `details` stay snake_case on the wire and are NOT case-converted.
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

    /// The operation is disabled in this build (e.g. data reset when `testingMode` is off).
    #[error("Operation not allowed: {0}")]
    OperationNotAllowed(String),

    /// FR-20: an edit/delete was refused by a guard. `code` is a stable reason code (LLD 14.4).
    #[error("Bill locked: {message}")]
    BillLocked { code: String, message: String },

    /// FR-45 / FR-46: a new record's name equals a deleted one. The UI offers Restore for
    /// `existing_id` instead of a raw constraint error. `code` is e.g. `ITEM_NAME_DELETED`.
    #[error("{message}")]
    DeletedNameClash { field: String, code: String, existing_id: i64, message: String },
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

    #[test]
    fn test_serialized_shape_of_every_variant() {
        use serde_json::json;
        let cases = vec![
            (AppError::ValidationError { field: "f".into(), message: "m".into() },
             json!({"type":"ValidationError","details":{"field":"f","message":"m"}})),
            (AppError::InsufficientStockError { item_id: 1, item_name: "Rice".into(), required: 5.0, available: 2.0 },
             json!({"type":"InsufficientStockError","details":{"item_id":1,"item_name":"Rice","required":5.0,"available":2.0}})),
            (AppError::not_found("Item", "9"),
             json!({"type":"NotFound","details":{"entity":"Item","id":"9"}})),
            (AppError::ConcurrencyConflict("c".into()), json!({"type":"ConcurrencyConflict","details":"c"})),
            (AppError::DatabaseError("d".into()), json!({"type":"DatabaseError","details":"d"})),
            (AppError::SearchIndexError("s".into()), json!({"type":"SearchIndexError","details":"s"})),
            (AppError::MigrationError("g".into()), json!({"type":"MigrationError","details":"g"})),
            (AppError::IoError("i".into()), json!({"type":"IoError","details":"i"})),
            (AppError::OperationNotAllowed("o".into()), json!({"type":"OperationNotAllowed","details":"o"})),
            (AppError::BillLocked { code: "LINKED_RETURNS".into(), message: "m".into() },
             json!({"type":"BillLocked","details":{"code":"LINKED_RETURNS","message":"m"}})),
            (AppError::DeletedNameClash { field: "name".into(), code: "ITEM_NAME_DELETED".into(), existing_id: 7, message: "m".into() },
             json!({"type":"DeletedNameClash","details":{"field":"name","code":"ITEM_NAME_DELETED","existing_id":7,"message":"m"}})),
        ];
        for (err, expected) in cases {
            assert_eq!(serde_json::to_value(&err).unwrap(), expected);
            let back: AppError = serde_json::from_value(expected).unwrap();
            assert_eq!(back, err);
        }
    }
}
