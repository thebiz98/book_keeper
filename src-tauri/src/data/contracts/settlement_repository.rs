use crate::core::entities::PaymentSettlement;
use crate::core::error::AppError;

pub trait SettlementRepository {
    fn create_settlement(&mut self, settlement: &PaymentSettlement) -> Result<i64, AppError> {
        self.create(settlement)
    }
    fn create(&mut self, settlement: &PaymentSettlement) -> Result<i64, AppError> {
        self.create_settlement(settlement)
    }
    fn list_by_party(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.list_by_party_id(party_id)
    }
    fn list_by_party_id(&self, party_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        self.list_by_party(party_id)
    }
    fn list_by_transaction(&self, _tx_type: &str, _tx_id: i64) -> Result<Vec<PaymentSettlement>, AppError> {
        Ok(Vec::new())
    }
    fn get_daily_settlement_total(&self, _date: &str, _tx_type: &str) -> Result<f64, AppError> {
        Ok(0.0)
    }
}
