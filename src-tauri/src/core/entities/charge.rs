use serde::{Deserialize, Serialize};

use crate::core::value_objects::{ChargeAppliesTo, ChargeEffect, TransactionType};

/// Reusable charge/discount master row (FR-19). Names are unique case-insensitively.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChargeType {
    pub id: i64,
    pub name: String,
    pub effect: ChargeEffect,
    pub applies_to: ChargeAppliesTo,
    pub default_amount: Option<f64>,
    pub is_active: bool,
    pub created_at: String,
}

/// One charge or discount line on a bill; `charge_name` and `effect` are snapshots (DD-30).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillCharge {
    pub id: i64,
    pub transaction_type: TransactionType,
    pub transaction_id: i64,
    pub charge_type_id: i64,
    pub charge_name: String,
    pub effect: ChargeEffect,
    pub amount: f64,
}
