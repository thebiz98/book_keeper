use std::collections::HashMap;
use std::sync::Arc;
use crate::core::dtos::{
    CashFlowReport, DashboardMetrics, LowStockItem, PnLReport, StockValuationReport, TrendDataPoint,
};
use crate::core::error::AppError;
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::report_service::ReportService;

#[derive(Clone)]
pub struct ReportServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
}

impl<U: UnitOfWork> ReportServiceImpl<U> {
    pub fn new(uow: U) -> Self {
        Self { uow }
    }
}

impl<U: UnitOfWork> ReportService for ReportServiceImpl<U> {
    fn get_dashboard_metrics(&self) -> Result<DashboardMetrics, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let sales = tx.sales().list_sales(None, None, None, None, 0, usize::MAX)?;
        let purchases = tx.purchases().list_purchases(None, None, None, None, 0, usize::MAX)?;
        let lots = tx.purchases().list_all_lots()?;
        let allocations = tx.allocations().list_all()?;
        let settlements = tx.settlements().list_all()?;

        let today_sales: f64 = sales.iter().map(|s| s.total_amount).sum();
        let today_purchases: f64 = purchases.iter().map(|p| p.total_amount).sum();
        let total_cogs: f64 = allocations.iter().map(|a| a.quantity_depleted * a.unit_cost).sum();
        let today_net_profit = today_sales - total_cogs;

        let total_stock_valuation: f64 = lots
            .iter()
            .map(|l| l.remaining_quantity * l.unit_cost)
            .sum();

        let customer_receivables: f64 = sales.iter().map(|s| s.balance_due).sum();
        let vendor_payables: f64 = purchases.iter().map(|p| p.balance_due).sum();

        let sales_cash: f64 = sales
            .iter()
            .filter(|s| s.payment_mode.as_deref() == Some("CASH"))
            .map(|s| s.paid_amount)
            .sum();
        let pur_cash: f64 = purchases
            .iter()
            .filter(|p| p.payment_mode.as_deref() == Some("CASH"))
            .map(|p| p.paid_amount)
            .sum();

        let refunds_cash: f64 = settlements
            .iter()
            .filter(|s| s.transaction_type == "SALE_REFUND" && s.payment_mode.as_deref() == Some("CASH"))
            .map(|s| s.amount_paid.abs())
            .sum();

        let customer_dues_cash: f64 = settlements
            .iter()
            .filter(|s| s.transaction_type == "CUSTOMER_SETTLEMENT" && s.payment_mode.as_deref() == Some("CASH"))
            .map(|s| s.amount_paid.abs())
            .sum();

        let vendor_dues_cash: f64 = settlements
            .iter()
            .filter(|s| s.transaction_type == "VENDOR_SETTLEMENT" && s.payment_mode.as_deref() == Some("CASH"))
            .map(|s| s.amount_paid.abs())
            .sum();

        let live_cash_in_drawer = sales_cash - pur_cash - refunds_cash + customer_dues_cash - vendor_dues_cash;

        Ok(DashboardMetrics {
            today_sales,
            today_purchases,
            today_sales_amount: today_sales,
            today_sales_count: sales.len(),
            today_purchase_amount: today_purchases,
            today_purchase_count: purchases.len(),
            today_net_profit,
            total_stock_valuation,
            customer_receivables,
            vendor_payables,
            live_cash_in_drawer,
        })
    }

    fn get_low_stock_alerts(&self) -> Result<Vec<LowStockItem>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let items = tx.items().list_catalog(None, true, 0, 50)?;
        Ok(items
            .into_iter()
            .map(|i| LowStockItem {
                id: i.id,
                item_id: i.id,
                name: i.name,
                category: i.category,
                current_stock: i.current_stock,
                base_unit: i.base_unit,
                default_sale_price: i.default_sale_price,
                mrp: i.mrp,
            })
            .collect())
    }

    fn get_sales_purchase_trend(&self, _period: &str) -> Result<Vec<TrendDataPoint>, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let sales = tx.sales().list_sales(None, None, None, None, 0, usize::MAX)?;
        let purchases = tx.purchases().list_purchases(None, None, None, None, 0, usize::MAX)?;
        let mut date_map: HashMap<String, (f64, f64)> = HashMap::new();
        for s in sales {
            let entry = date_map.entry(s.transaction_date).or_insert((0.0, 0.0));
            entry.0 += s.total_amount;
        }
        for p in purchases {
            let entry = date_map.entry(p.transaction_date).or_insert((0.0, 0.0));
            entry.1 += p.total_amount;
        }
        let mut trend: Vec<TrendDataPoint> = date_map
            .into_iter()
            .map(|(date, (sales_amount, purchase_amount))| TrendDataPoint {
                date,
                sales_amount,
                purchase_amount,
            })
            .collect();
        trend.sort_by(|a, b| a.date.cmp(&b.date));
        Ok(trend)
    }

    fn get_pnl_report(&self, from_date: &str, to_date: &str) -> Result<PnLReport, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let sales = tx.sales().list_sales(Some(from_date), Some(to_date), None, None, 0, usize::MAX)?;
        let allocations = tx.allocations().list_all()?;
        let gross_sales: f64 = sales.iter().map(|s| s.total_amount).sum();
        let cogs: f64 = allocations.iter().map(|a| a.quantity_depleted * a.unit_cost).sum();
        let net_profit = gross_sales - cogs;
        Ok(PnLReport {
            from_date: from_date.to_string(),
            to_date: to_date.to_string(),
            gross_sales,
            cogs,
            net_profit,
            total_discounts: 0.0,
            returns_refunded: 0.0,
        })
    }

    fn get_stock_valuation_report(&self) -> Result<StockValuationReport, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let lots = tx.purchases().list_all_lots()?;
        let total_valuation: f64 = lots
            .iter()
            .map(|l| l.remaining_quantity * l.unit_cost)
            .sum();
        let total_items_count = lots.len();
        Ok(StockValuationReport {
            total_valuation,
            active_lots_count: total_items_count,
            total_items_count,
            categories_breakdown: Vec::new(),
            categories: Vec::new(),
        })
    }

    fn get_cash_flow_report(&self, from_date: &str, to_date: &str) -> Result<CashFlowReport, AppError> {
        let mut tx = self.uow.begin_transaction()?;
        let settlements = tx.settlements().list_all()?;
        let mut total_inflow = 0.0;
        let mut total_outflow = 0.0;
        for s in settlements {
            if s.settlement_date.as_str() >= from_date && s.settlement_date.as_str() <= to_date {
                if s.amount_paid > 0.0 {
                    total_inflow += s.amount_paid;
                } else {
                    total_outflow += s.amount_paid.abs();
                }
            }
        }
        let net_cash_flow = total_inflow - total_outflow;
        Ok(CashFlowReport {
            from_date: from_date.to_string(),
            to_date: to_date.to_string(),
            total_inflow,
            total_outflow,
            net_cash_flow,
            closing_drawer_balance: net_cash_flow,
            sales_cash_inflow: total_inflow,
            customer_settlements_inflow: 0.0,
            purchase_cash_outflow: total_outflow,
            vendor_settlements_outflow: 0.0,
            refunds_cash_outflow: 0.0,
            net_cash_balance: net_cash_flow,
        })
    }
}
