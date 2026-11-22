//! Testing-only data reset (gated by `testingMode` in `app.config.json`).
use serde::{Deserialize, Serialize};

use crate::core::error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResetScope {
    /// Sales, purchases, payments, returns and adjustments go; stock returns to opening stock.
    /// Items, bundles, parties and the shop profile stay.
    Transactions,
    /// Everything except the shop profile and backup folder.
    All,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletedRows {
    pub table: String,
    pub rows: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResetSummary {
    pub scope: ResetScope,
    /// Rows deleted per table, in delete order (a table cleared in two steps appears twice).
    pub deleted: Vec<DeletedRows>,
    /// The pre-reset backup written before anything was deleted.
    pub backup_path: String,
}

pub trait DataResetService: Send + Sync {
    /// Refuses with `OperationNotAllowed` unless testing mode is on. Otherwise writes a pre-reset
    /// backup into `backup_dir` (aborting if that fails), then deletes in one transaction.
    fn reset(&self, scope: ResetScope, backup_dir: &str) -> Result<ResetSummary, AppError>;
}

impl<T: DataResetService + ?Sized> DataResetService for std::sync::Arc<T> {
    fn reset(&self, scope: ResetScope, backup_dir: &str) -> Result<ResetSummary, AppError> {
        (**self).reset(scope, backup_dir)
    }
}
