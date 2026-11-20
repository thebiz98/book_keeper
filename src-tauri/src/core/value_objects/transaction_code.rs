use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TransactionCode(pub String);

impl TransactionCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TransactionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TransactionCode {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TransactionCode {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}
