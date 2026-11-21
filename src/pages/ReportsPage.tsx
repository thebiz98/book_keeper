import React, { useState, useEffect } from 'react';
import { BarChart3 } from 'lucide-react';
import { api } from '@/api';
import type { CashFlowReport, PnlReport, StockValuationReport } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { Tabs } from '@/components/ui/Tabs';

export const ReportsPage: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'pnl' | 'valuation' | 'cashflow'>('pnl');
  const [datePreset, setDatePreset] = useState<'today' | 'week' | 'month' | 'year'>('month');

  // Reports data
  const [pnl, setPnl] = useState<PnlReport | null>(null);
  const [valuation, setValuation] = useState<StockValuationReport | null>(null);
  const [cashFlow, setCashFlow] = useState<CashFlowReport | null>(null);

  const fetchReports = () => {
    const today = new Date().toISOString().split('T')[0];
    const fromDate = '2026-11-01';
    const toDate = today;

    Promise.all([
      api.getPnLReport(fromDate, toDate),
      api.getStockValuationReport(),
      api.getCashFlowReport(fromDate, toDate),
    ])
      .then(([pnlData, valData, cfData]) => {
        setPnl(pnlData);
        setValuation(valData);
        setCashFlow(cfData);
      })
      .catch(() => {})
  };

  useEffect(() => {
    fetchReports();
  }, [datePreset]);

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        <div>
          <h1 className="text-xl font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
            <BarChart3 className="text-blue-600 dark:text-blue-400" size={24} />
            Financial & Inventory Accounting Reports
          </h1>
          <p className="text-xs text-slate-500 mt-1">
            Real-time P&L with strict FIFO cost of goods sold (COGS), inventory asset valuation, and cash flow.
          </p>
        </div>
        <div className="flex items-center gap-2">
          {/* Preset buttons */}
          <div className="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 p-1 rounded-md text-xs font-medium">
            {(['today', 'week', 'month', 'year'] as const).map((p) => (
              <button
                key={p}
                onClick={() => setDatePreset(p)}
                className={`px-2.5 py-1 rounded transition-colors capitalize ${
                  datePreset === p
                    ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 font-semibold shadow-2xs'
                    : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
                }`}
              >
                {p === 'year' ? 'This FY' : `This ${p}`}
              </button>
            ))}
          </div>
        </div>
      </div>

      {/* Tabs */}
      <Tabs
        tabs={[
          { id: 'pnl', label: 'Profit & Loss (FIFO COGS)' },
          { id: 'valuation', label: 'Stock Valuation (FIFO Lots)' },
          { id: 'cashflow', label: 'Cash Flow Statement' },
        ]}
        activeTab={activeTab}
        onChange={(id) => setActiveTab(id as any)}
      />

      {/* Tab 1: P&L Statement */}
      {activeTab === 'pnl' && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Gross Sales Revenue</span>
              <div className="text-2xl font-bold mt-1 text-slate-900 dark:text-slate-100">
                <CurrencyDisplay amount={pnl?.grossSales ?? 0} />
              </div>
              <span className="text-xs text-slate-400 mt-1 block">Total customer billings</span>
            </div>

            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Cost of Goods Sold (COGS)</span>
              <div className="text-2xl font-bold mt-1 text-amber-600 dark:text-amber-400">
                <CurrencyDisplay amount={pnl?.cogs ?? 0} />
              </div>
              <span className="text-xs text-slate-400 mt-1 block">FIFO batch cost depletion</span>
            </div>

            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Net Operating Profit</span>
              <div className="text-2xl font-bold mt-1">
                <CurrencyDisplay amount={pnl?.netProfit ?? 0} colored size="hero" />
              </div>
              <span className="text-xs text-emerald-600 dark:text-emerald-400 mt-1 block">
                {pnl?.grossSales ? `Margin: ${Math.round(((pnl.netProfit ?? 0) / pnl.grossSales) * 100)}%` : 'Active'}
              </span>
            </div>
          </div>

          {/* Detailed Statement Breakdown */}
          <div className="bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-800 p-6 shadow-xs max-w-2xl">
            <h3 className="text-sm font-semibold text-slate-800 dark:text-slate-200 uppercase tracking-wider mb-4 border-b pb-2 dark:border-slate-800">
              P&L Ledger Breakdown
            </h3>
            <div className="space-y-3 text-sm">
              <div className="flex justify-between items-center py-1">
                <span className="text-slate-600 dark:text-slate-300">1. Gross Revenue (Counter Sales)</span>
                <span className="font-mono font-semibold">
                  <CurrencyDisplay amount={pnl?.grossSales ?? 0} />
                </span>
              </div>
              <div className="flex justify-between items-center py-1 text-slate-500">
                <span>Less: Trade Discounts</span>
                <span className="font-mono text-rose-500">
                  -<CurrencyDisplay amount={pnl?.totalDiscounts ?? 0} />
                </span>
              </div>
              <div className="flex justify-between items-center py-1 text-slate-500">
                <span>Less: Customer Cash Refunds</span>
                <span className="font-mono text-rose-500">
                  -<CurrencyDisplay amount={pnl?.returnsRefunded ?? 0} />
                </span>
              </div>
              <div className="border-t border-slate-200 dark:border-slate-800 pt-2 flex justify-between items-center font-medium">
                <span>Net Sales Revenue</span>
                <span className="font-mono font-semibold">
                  <CurrencyDisplay
                    amount={(pnl?.grossSales ?? 0) - (pnl?.totalDiscounts ?? 0) - (pnl?.returnsRefunded ?? 0)}
                  />
                </span>
              </div>
              <div className="flex justify-between items-center py-1 text-slate-500">
                <span>Less: Cost of Goods Sold (COGS FIFO)</span>
                <span className="font-mono text-rose-500">
                  -<CurrencyDisplay amount={pnl?.cogs ?? 0} />
                </span>
              </div>
              <div className="border-t-2 border-slate-900 dark:border-slate-100 pt-3 flex justify-between items-center font-bold text-base">
                <span>Net Operating Profit</span>
                <span className="font-mono text-emerald-600 dark:text-emerald-400">
                  <CurrencyDisplay amount={pnl?.netProfit ?? 0} colored />
                </span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tab 2: Stock Valuation */}
      {activeTab === 'valuation' && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Total Stock Asset Valuation</span>
              <div className="text-2xl font-bold mt-1 text-blue-600 dark:text-blue-400">
                <CurrencyDisplay amount={valuation?.totalValuation ?? 0} size="hero" />
              </div>
              <span className="text-xs text-slate-400 mt-1 block">Sum of FIFO cost × remaining lots</span>
            </div>

            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Active Catalog Items</span>
              <div className="text-2xl font-bold mt-1 text-slate-900 dark:text-slate-100">
                {valuation?.totalItemsCount ?? 0}
              </div>
              <span className="text-xs text-slate-400 mt-1 block">Stationery master products</span>
            </div>

            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Inward Batches Tracked</span>
              <div className="text-2xl font-bold mt-1 text-slate-900 dark:text-slate-100">
                {valuation?.activeLotsCount ?? 0}
              </div>
              <span className="text-xs text-slate-400 mt-1 block">Distinct purchase lot layers</span>
            </div>
          </div>

          <div className="bg-white dark:bg-slate-900 p-6 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
            <h3 className="text-sm font-semibold text-slate-800 dark:text-slate-200 uppercase tracking-wider mb-2">
              Valuation Methodology
            </h3>
            <p className="text-xs text-slate-500 leading-relaxed">
              Book Keeper calculates inventory valuation using strict First-In, First-Out (FIFO) lot depletion.
              Each purchase batch creates a discrete cost layer in SQLite. When items are billed, units are depleted
              from the oldest remaining batch, ensuring 100% tax and audit compliance without weighted average distortion.
            </p>
          </div>
        </div>
      )}

      {/* Tab 3: Cash Flow */}
      {activeTab === 'cashflow' && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Cash Inflows</span>
              <div className="text-2xl font-bold mt-1 text-emerald-600 dark:text-emerald-400">
                <CurrencyDisplay amount={cashFlow?.totalInflow ?? 0} />
              </div>
              <span className="text-xs text-slate-400 mt-1 block">Counter cash + customer dues</span>
            </div>

            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Cash Outflows</span>
              <div className="text-2xl font-bold mt-1 text-rose-600 dark:text-rose-400">
                <CurrencyDisplay amount={cashFlow?.totalOutflow ?? 0} />
              </div>
              <span className="text-xs text-slate-400 mt-1 block">Inward purchases + vendor dues</span>
            </div>

            <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
              <span className="text-xs font-medium text-slate-500">Net Drawer Cash Flow</span>
              <div className="text-2xl font-bold mt-1">
                <CurrencyDisplay amount={cashFlow?.netCashFlow ?? 0} colored size="hero" />
              </div>
              <span className="text-xs text-slate-400 mt-1 block">Live net liquidity</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};

export default ReportsPage;
