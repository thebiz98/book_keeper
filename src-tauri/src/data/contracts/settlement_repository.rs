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
    /// FR-20: deletes the bill's rows of the given kinds and returns them.
    fn delete_by_transaction(
        &mut self,
        tx_type: TransactionType,
        tx_id: i64,
        kinds: &[PaymentKind],
    ) -> Result<Vec<PaymentSettlement>, AppError>;
    /// FR-20: re-points the bill's rows of the given kinds to another party.
    fn update_party_for_transaction(
        &mut self,
        tx_type: TransactionType,
        tx_id: i64,
        party_id: Option<i64>,
        kinds: &[PaymentKind],
    ) -> Result<(), AppError>;
    /// Distinct `return_id`s of the bill's `RETURN_CREDIT` rows.
    fn return_credit_return_ids(&self, tx_type: TransactionType, tx_id: i64) -> Result<Vec<i64>, AppError>;
    /// FR-20: deletes every ledger row booked by one return and returns them.
    fn delete_by_return(&mut self, return_id: i64) -> Result<Vec<PaymentSettlement>, AppError>;
}
