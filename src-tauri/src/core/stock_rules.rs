//! The single definition of the low-stock rule (Rust form and its SQL twin).
use crate::core::error::AppError;
use crate::core::value_objects::ItemType;

/// Default per-item low stock level in base units.
pub const DEFAULT_LOW_STOCK_THRESHOLD: f64 = 5.0;

/// SQL twin of [`is_low_stock`]; evaluated against the `items` row being updated.
pub const LOW_STOCK_SQL: &str = "(item_type = 'PHYSICAL' AND current_stock < low_stock_threshold)";

pub fn default_low_stock_threshold() -> f64 {
    DEFAULT_LOW_STOCK_THRESHOLD
}

/// Low when PHYSICAL and stock is strictly below the level. Level 0 never alerts.
pub fn is_low_stock(item_type: ItemType, current_stock: f64, threshold: f64) -> bool {
    item_type == ItemType::Physical && current_stock < threshold
}

pub fn validate_low_stock_threshold(t: f64) -> Result<(), AppError> {
    if t.is_finite() && t >= 0.0 {
        Ok(())
    } else {
        Err(AppError::ValidationError {
            field: "low_stock_threshold".into(),
            message: "Low stock level must be a number that is 0 or more".into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truth_table() {
        assert!(is_low_stock(ItemType::Physical, 4.0, 5.0));
        assert!(!is_low_stock(ItemType::Physical, 5.0, 5.0));
        assert!(!is_low_stock(ItemType::Physical, 0.0, 0.0));
        assert!(!is_low_stock(ItemType::Service, 0.0, 5.0));
    }

    #[test]
    fn validation() {
        assert!(validate_low_stock_threshold(0.0).is_ok());
        assert!(validate_low_stock_threshold(2.5).is_ok());
        assert!(validate_low_stock_threshold(-1.0).is_err());
        assert!(validate_low_stock_threshold(f64::NAN).is_err());
        assert!(validate_low_stock_threshold(f64::INFINITY).is_err());
    }
}
