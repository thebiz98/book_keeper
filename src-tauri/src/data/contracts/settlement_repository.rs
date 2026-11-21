use crate::core::dtos::LedgerTotals;
use crate::core::entities::PaymentSettlement;
use crate::core::error::AppError;
use crate::core::value_objects::{PaymentKind, TransactionType};

pub trait SettlementRepository {
    fn create_settlement(&mut self, settlement: &PaymentSettlement) -> Result<i64, AppError>;
    fn list_by_party(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError>;
    fn list_by_transaction(&self, tx_type: TransactionType, tx_id: i64) -> Result<Vec<PaymentSettlement>, AppError>;
    /// Σ `amount_paid` for one bill grouped by `kind` (recomputes the cached header invariant).
    fn sum_by_kind_for_transaction(&self, tx_type: TransactionType, tx_id: i64) -> Result<LedgerTotals, AppError>;
    /// Σ `amount_paid` of rows with `from <= settlement_date <= to`, grouped by
    /// `(transaction_type, kind)`. The single query behind the cash drawer / cash flow (DD-01).
    fn sum_by_type_kind_between(
        &self,
        from: &str,
        to: &str,
    ) -> Result<Vec<(TransactionType, PaymentKind, f64)>, AppError>;
    /// Every ledger row (drawer and cash-flow reports; F5 replaces this with SQL aggregates).
    fn list_all(&self) -> Result<Vec<PaymentSettlement>, AppError>;
}
