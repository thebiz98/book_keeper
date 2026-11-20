use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReturnType {
    Customer,
    Vendor,
    CustomerReturn,
    VendorReturn,
}
