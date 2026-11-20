use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShopSetting {
    pub id: i64,
    pub shop_name: String,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub tax_id: Option<String>, // GSTIN / PAN
}
