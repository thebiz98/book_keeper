use serde::{Deserialize, Serialize};

use crate::core::value_objects::{ChargeAppliesTo, ChargeEffect};

/// One charge/discount line submitted with a bill; name and effect come from the type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillChargeInput {
    pub charge_type_id: i64,
    pub amount: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreateChargeTypePayload {
    pub name: String,
    pub effect: ChargeEffect,
    pub applies_to: ChargeAppliesTo,
    #[serde(default)]
    pub default_amount: Option<f64>,
}
