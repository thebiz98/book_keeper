//! DD-56 previews: dry runs of the real commands (the transaction is dropped, never committed).
//! The UI shows these figures before Save and never computes a business rule itself.
use serde::{Deserialize, Serialize};
use crate::core::dtos::charge_dto::BillChargeInput;
use crate::core::dtos::purchase_dto::{CreatePurchaseItemPayload, CreatePurchasePayload};
use crate::core::dtos::sale_dto::{CreateSaleItemPayload, CreateSalePayload};
use crate::core::error::AppError;
use crate::core::value_objects::PaymentStatus;

/// One problem a preview found: the same `field` and `message` the real command would raise.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewError {
    pub field: String,
    pub message: String,
}

impl PreviewError {
    /// Maps a command error to data. Business errors become a `PreviewError`; infrastructure
    /// errors (database, IO ...) stay failures (`None`).
    pub fn from_app_error(e: &AppError) -> Option<Self> {
        match e {
            AppError::ValidationError { field, message } => {
                Some(Self { field: field.clone(), message: message.clone() })
            }
            AppError::BillLocked { code, message } => Some(Self { field: code.clone(), message: message.clone() }),
            AppError::InsufficientStockError { .. } => {
                Some(Self { field: "items".to_string(), message: e.to_string() })
            }
            AppError::NotFound { .. } => Some(Self { field: "id".to_string(), message: e.to_string() }),
            _ => None,
        }
    }
}

/// What the bill would look like once saved with the chosen payment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillPaymentPreview {
    pub paid_amount: f64,
    pub balance_due: f64,
    pub change_amount: f64,
    pub payment_status: PaymentStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BillPreview {
    pub line_totals: Vec<f64>,
    pub items_subtotal: f64,
    pub charges_total: f64,
    pub discounts_total: f64,
    pub total_amount: f64,
    /// Settlements and return credits already applied to the bill (edit only; 0 on create).
    pub applied_amount: f64,
    /// What the payment must cover: `total - applied` (the whole total on create).
    pub open_amount: f64,
    /// `None` when the payload carried no payment status (totals and stock checks only).
    pub payment: Option<BillPaymentPreview>,
    pub errors: Vec<PreviewError>,
}

/// `CreateSalePayload` with the payment optional (the cart shows totals before a status is chosen).
/// FR-47: one sale entry-row line, for the Total box before Add (no database).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewSaleLinePayload {
    pub bundle_quantity: f64,
    pub unit_price: f64,
    /// The total the cashier typed; `None` = untouched.
    #[serde(default)]
    pub entered_total: Option<f64>,
}

/// DD-82: `unit_price` is the rate the line is added at (`entered_total ÷ qty`, not rounded, when a
/// total was typed; otherwise the given rate). `line_total = round2(qty × unit_price)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SaleLinePreview {
    pub line_total: f64,
    pub unit_price: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewSalePayload {
    pub transaction_date: String,
    pub party_id: Option<i64>,
    #[serde(default)]
    pub payment_status: Option<PaymentStatus>,
    #[serde(default)]
    pub paid_amount: Option<f64>,
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<CreateSaleItemPayload>,
    #[serde(default)]
    pub charges: Vec<BillChargeInput>,
}

impl PreviewSalePayload {
    /// The command payload plus whether the payment is to be checked.
    pub fn into_command(self) -> (CreateSalePayload, bool) {
        let with_payment = self.payment_status.is_some();
        (
            CreateSalePayload {
                transaction_date: self.transaction_date,
                party_id: self.party_id,
                payment_status: self.payment_status.unwrap_or(PaymentStatus::Unpaid),
                paid_amount: self.paid_amount.unwrap_or(0.0),
                payment_mode: self.payment_mode,
                notes: self.notes,
                items: self.items,
                charges: self.charges,
            },
            with_payment,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreviewPurchasePayload {
    pub transaction_date: String,
    pub party_id: Option<i64>,
    #[serde(default)]
    pub supplier_invoice_ref: Option<String>,
    #[serde(default)]
    pub payment_status: Option<PaymentStatus>,
    #[serde(default)]
    pub paid_amount: Option<f64>,
    pub payment_mode: Option<String>,
    pub notes: Option<String>,
    pub items: Vec<CreatePurchaseItemPayload>,
    #[serde(default)]
    pub charges: Vec<BillChargeInput>,
}

impl PreviewPurchasePayload {
    pub fn into_command(self) -> (CreatePurchasePayload, bool) {
        let with_payment = self.payment_status.is_some();
        (
            CreatePurchasePayload {
                transaction_date: self.transaction_date,
                party_id: self.party_id,
                supplier_invoice_ref: self.supplier_invoice_ref,
                payment_status: self.payment_status.unwrap_or(PaymentStatus::Unpaid),
                paid_amount: self.paid_amount.unwrap_or(0.0),
                payment_mode: self.payment_mode,
                notes: self.notes,
                items: self.items,
                charges: self.charges,
            },
            with_payment,
        )
    }
}

/// Preview of `create_adjustment`: the unit cost it would use and its value effect.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StockAdjustmentPreview {
    pub previous_stock: f64,
    pub new_stock: f64,
    /// Lot cost used by a positive adjustment (explicit or the latest-lot default); `None` for a write-off.
    pub unit_cost: Option<f64>,
    /// Σ qty x lot cost of a write-off (FIFO); 0 for a positive adjustment.
    pub loss_value: f64,
    /// `+qty x unit_cost` when adding, `-loss_value` when writing off.
    pub value_change: f64,
}
