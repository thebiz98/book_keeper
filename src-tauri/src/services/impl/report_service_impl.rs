//! Dashboard & reports (LLD §6.10). Every figure is computed by SQL aggregates in
//! `ReportRepository` inside one read snapshot; cash comes from the single
//! `accounting_service_impl::ledger_cash_summary`.
use std::sync::Arc;

use chrono::{Datelike, Duration, NaiveDate};

use crate::core::dtos::{
    CashFlowReport, DashboardMetrics, LowStockItem, PnLReport, RecentTransaction,
    StockValuationReport, TrendDataPoint,
};
use crate::core::error::AppError;
use crate::core::money::{check_date, round2};
use crate::data::contracts::unit_of_work::UnitOfWork;
use crate::services::contracts::report_service::ReportService;
use crate::services::r#impl::accounting_service_impl::ledger_cash_summary;

const MAX_RECENT: usize = 100;

#[derive(Clone)]
pub struct ReportServiceImpl<U: UnitOfWork = Arc<dyn UnitOfWork>> {
    pub uow: U,
    /// Fixed business date for deterministic tests; `None` = local clock.
    fixed_today: Option<NaiveDate>,
}

impl<U: UnitOfWork> ReportServiceImpl<U> {
    pub fn new(uow: U) -> Self {
        Self { uow, fixed_today: None }
    }

    /// Pins "today" ("YYYY-MM-DD") for dashboard and trend (tests, back-dated reports).
    pub fn with_fixed_today(uow: U, today: &str) -> Self {
        let d = NaiveDate::parse_from_str(today, "%Y-%m-%d").expect("fixed today must be YYYY-MM-DD");
        Self { uow, fixed_today: Some(d) }
    }

    fn today(&self) -> NaiveDate {
        self.fixed_today.unwrap_or_else(|| chrono::Local::now().date_naive())
    }
}

fn fmt(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn validation(field: &str, message: impl Into<String>) -> AppError {
    AppError::ValidationError { field: field.to_string(), message: message.into() }
}

fn check_range(from: &str, to: &str) -> Result<(), AppError> {
    check_date("from_date", from)?;
    check_date("to_date", to)?;
    if from > to {
        return Err(validation("from_date", "from_date must not be after to_date"));
    }
    Ok(())
}

impl<U: UnitOfWork> ReportService for ReportServiceImpl<U> {
    fn get_dashboard_metrics(&self) -> Result<DashboardMetrics, AppError> {
        let today = fmt(self.today());
        let mut tx = self.uow.begin_read()?;
        let pnl = tx.reports().pnl(&today, &today)?;
        let (today_sales_amount, today_sales_count) = tx.reports().sales_totals(&today, &today)?;
        let (today_purchase_amount, today_purchase_count) = tx.reports().purchase_totals(&today, &today)?;
        let (customer_receivables, vendor_payables) = tx.reports().dues_totals()?;
        let total_stock_valuation = tx.reports().stock_valuation()?.total_valuation;
        let live_cash_in_drawer = ledger_cash_summary(&mut *tx, &today, &today)?.net;
        Ok(DashboardMetrics {
            today_sales_amount,
            today_sales_count,
            today_purchase_amount,
            today_purchase_count,
            today_net_profit: pnl.net_profit,
            total_stock_valuation,
            customer_receivables,
            vendor_payables,
            live_cash_in_drawer,
        })
    }

    /// Returns every active low-stock item (no cap), lowest stock first.
    fn get_low_stock_alerts(&self) -> Result<Vec<LowStockItem>, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.reports().low_stock_items()
    }

    fn get_sales_purchase_trend(&self, period: &str) -> Result<Vec<TrendDataPoint>, AppError> {
        let today = self.today();
        let (from, key_len, labels): (NaiveDate, usize, Vec<String>) = match period {
            "7d" | "30d" => {
                let days = if period == "7d" { 7 } else { 30 };
                let from = today - Duration::days(days - 1);
                (from, 10, (0..days).map(|i| fmt(from + Duration::days(i))).collect())
            }
            "1y" => {
                let first = NaiveDate::from_ymd_opt(today.year(), today.month(), 1)
                    .ok_or_else(|| validation("period", "invalid date"))?;
                let mut months = Vec::with_capacity(12);
                let (mut y, mut m) = (first.year(), first.month() as i32);
                for _ in 0..12 {
                    months.push(format!("{y:04}-{m:02}-01"));
                    m -= 1;
                    if m == 0 {
                        m = 12;
                        y -= 1;
                    }
                }
                months.reverse();
                let from = NaiveDate::parse_from_str(&months[0], "%Y-%m-%d")
                    .map_err(|_| validation("period", "invalid date"))?;
                (from, 7, months)
            }
            other => {
                return Err(validation("period", format!("period must be '7d', '30d' or '1y', got '{other}'")))
            }
        };
        let mut tx = self.uow.begin_read()?;
        let buckets = tx.reports().trend_buckets(&fmt(from), &fmt(today), key_len)?;
        Ok(labels
            .into_iter()
            .map(|date| {
                let key = &date[..key_len];
                let (s, p) = buckets
                    .iter()
                    .find(|(k, _, _)| k == key)
                    .map(|(_, s, p)| (*s, *p))
                    .unwrap_or((0.0, 0.0));
                TrendDataPoint { date, sales_amount: s, purchase_amount: p }
            })
            .collect())
    }

    fn get_pnl_report(&self, from_date: &str, to_date: &str) -> Result<PnLReport, AppError> {
        check_range(from_date, to_date)?;
        let mut tx = self.uow.begin_read()?;
        tx.reports().pnl(from_date, to_date)
    }

    fn get_stock_valuation_report(&self) -> Result<StockValuationReport, AppError> {
        let mut tx = self.uow.begin_read()?;
        tx.reports().stock_valuation()
    }

    fn get_cash_flow_report(&self, from_date: &str, to_date: &str) -> Result<CashFlowReport, AppError> {
        check_range(from_date, to_date)?;
        let mut tx = self.uow.begin_read()?;
        let s = ledger_cash_summary(&mut *tx, from_date, to_date)?;
        Ok(CashFlowReport {
            sales_cash_inflow: round2(s.sale_upfront),
            customer_settlements_inflow: round2(s.sale_settlement),
            vendor_refunds_inflow: round2(s.vendor_refund_in),
            purchase_cash_outflow: round2(s.purchase_upfront),
            vendor_settlements_outflow: round2(s.purchase_settlement),
            refunds_cash_outflow: round2(s.customer_refund_out),
            net_cash_balance: s.net,
        })
    }

    fn get_recent_transactions(&self, limit: usize) -> Result<Vec<RecentTransaction>, AppError> {
        if limit == 0 || limit > MAX_RECENT {
            return Err(validation("limit", format!("limit must be between 1 and {MAX_RECENT}")));
        }
        let mut tx = self.uow.begin_read()?;
        tx.reports().recent_transactions(limit)
    }
}
