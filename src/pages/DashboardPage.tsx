import React, { useState, useEffect } from 'react';
import {
  TrendingUp,
  AlertTriangle,
  Users,
  Package,
  ShoppingCart,
  Truck,
  BookOpen,
  ArrowUpRight,
} from 'lucide-react';
import { api } from '@/api';
import type { DashboardMetrics, LowStockItem, SaleHistoryRow, TrendDataPoint } from '@/types';
import { useAppStore } from '@/store/useAppStore';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { StockStatusBadge } from '@/components/common/StockStatusBadge';
import { PaymentStatusBadge } from '@/components/common/PaymentStatusBadge';
import { Card, CardContent } from '@/components/ui/Card';
import { Button } from '@/components/ui/Button';

export const DashboardPage: React.FC = () => {
  const setActiveRoute = useAppStore((s) => s.setActiveRoute);
  const liveCashInDrawer = useAppStore((s) => s.liveCashInDrawer);

  const [metrics, setMetrics] = useState<DashboardMetrics | null>(null);
  const [lowStockAlerts, setLowStockAlerts] = useState<LowStockItem[]>([]);
  const [trendData, setTrendData] = useState<TrendDataPoint[]>([]);
  // TODO(F6-c): use getRecentTransactions (mixed sales/purchases/returns) and drop the parties lookup.
  const [recentSales, setRecentSales] = useState<SaleHistoryRow[]>([]);
  const [customerNames, setCustomerNames] = useState<Record<number, string>>({});

  useEffect(() => {
    let isMounted = true;

    Promise.all([
      api.getDashboardMetrics().catch(() => null),
      api.getLowStockAlerts().catch(() => [] as LowStockItem[]),
      api.getSalesPurchaseTrend('7d').catch(() => [] as TrendDataPoint[]),
      api.getSalesHistory({ limit: 6 }).catch(() => null),
      api.listParties('CUSTOMER', false).catch(() => []),
    ]).then(([metricsRes, alertsRes, trendRes, salesRes, customers]) => {
      if (!isMounted) return;
      if (metricsRes) setMetrics(metricsRes);
      setLowStockAlerts(alertsRes);
      setTrendData(trendRes);
      if (salesRes) setRecentSales(salesRes.items);
      setCustomerNames(Object.fromEntries(customers.map((c) => [c.id, c.name])));
    });

    return () => {
      isMounted = false;
    };
  }, []);

  const todayGross = metrics?.todaySalesAmount ?? 0;
  const receivables = metrics?.customerReceivables ?? 0;
  const netProfit = metrics?.todayNetProfit ?? 0;
  const stockValuation = metrics?.totalStockValuation ?? 0;

  return (
    <div className="flex flex-col gap-4">
      {/* Action Header Ribbon */}
      <div className="flex flex-wrap items-center justify-between gap-3 bg-white dark:bg-slate-900 p-3.5 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        <div>
          <h1 className="text-base font-bold text-slate-900 dark:text-slate-100">
            Retail Counter Cockpit
          </h1>
          <p className="text-xs text-slate-500">
            Real-time daily turnover, cash drawer, and inventory health
          </p>
        </div>

        <div className="flex items-center gap-2">
          <Button
            variant="primary"
            size="sm"
            onClick={() => setActiveRoute('sale')}
            hotkey="F2"
            leftIcon={<ShoppingCart className="h-3.5 w-3.5" />}
          >
            New Sale
          </Button>

          <Button
            variant="secondary"
            size="sm"
            onClick={() => setActiveRoute('purchase')}
            hotkey="F3"
            leftIcon={<Truck className="h-3.5 w-3.5" />}
          >
            New Purchase
          </Button>

          <Button
            variant="outline"
            size="sm"
            onClick={() => setActiveRoute('dues')}
            hotkey="F5"
            leftIcon={<BookOpen className="h-3.5 w-3.5" />}
          >
            Settle Due
          </Button>
        </div>
      </div>

      {/* KPI 6-Grid Ribbon */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        {/* 1. Today's Gross Sales */}
        <Card>
          <CardContent className="p-3 flex flex-col justify-between h-full">
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
              Today&apos;s Gross Sales
            </span>
            <div className="text-lg font-bold font-mono text-slate-900 dark:text-slate-100 my-1">
              <CurrencyDisplay amount={todayGross} size="lg" />
            </div>
            <span className="text-[10px] text-emerald-600 font-medium flex items-center gap-0.5">
              <TrendingUp className="h-3 w-3" /> Retail Counter
            </span>
          </CardContent>
        </Card>

        {/* 2. Cash in Drawer */}
        <Card>
          <CardContent className="p-3 flex flex-col justify-between h-full">
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
              Cash in Drawer
            </span>
            <div className="text-lg font-bold font-mono my-1">
              <CurrencyDisplay amount={liveCashInDrawer} colored size="lg" />
            </div>
            <span className="text-[10px] text-slate-500 font-mono">Live physical cash</span>
          </CardContent>
        </Card>

        {/* 3. Low Stock Items */}
        <Card
          className="cursor-pointer hover:border-amber-400 transition-colors"
          onClick={() => setActiveRoute('inventory')}
        >
          <CardContent className="p-3 flex flex-col justify-between h-full">
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
              Low Stock Alerts
            </span>
            <div className="text-lg font-bold font-mono text-amber-600 dark:text-amber-400 my-1">
              {lowStockAlerts.length} Items
            </div>
            <span className="text-[10px] text-amber-600 flex items-center gap-0.5 font-medium">
              <AlertTriangle className="h-3 w-3" /> &lt; 5 units threshold
            </span>
          </CardContent>
        </Card>

        {/* 4. Customer Receivables */}
        <Card
          className="cursor-pointer hover:border-rose-400 transition-colors"
          onClick={() => setActiveRoute('dues')}
        >
          <CardContent className="p-3 flex flex-col justify-between h-full">
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
              Khata Receivables
            </span>
            <div className="text-lg font-bold font-mono text-rose-600 dark:text-rose-400 my-1">
              <CurrencyDisplay amount={receivables} size="lg" />
            </div>
            <span className="text-[10px] text-slate-500 font-medium flex items-center gap-0.5">
              <Users className="h-3 w-3" /> Customer credit
            </span>
          </CardContent>
        </Card>

        {/* 5. Today's Net Operating Profit */}
        <Card>
          <CardContent className="p-3 flex flex-col justify-between h-full">
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
              Today&apos;s Net Profit
            </span>
            <div className="text-lg font-bold font-mono text-emerald-600 dark:text-emerald-400 my-1">
              <CurrencyDisplay amount={netProfit} colored size="lg" />
            </div>
            <span className="text-[10px] text-slate-500 font-mono">FIFO Gross - COGS</span>
          </CardContent>
        </Card>

        {/* 6. Inventory Valuation */}
        <Card>
          <CardContent className="p-3 flex flex-col justify-between h-full">
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
              Stock Valuation
            </span>
            <div className="text-lg font-bold font-mono text-slate-900 dark:text-slate-100 my-1">
              <CurrencyDisplay amount={stockValuation} size="lg" />
            </div>
            <span className="text-[10px] text-slate-500 font-mono flex items-center gap-0.5">
              <Package className="h-3 w-3" /> Active FIFO Lots
            </span>
          </CardContent>
        </Card>
      </div>

      {/* Main Split: Left 7-Day Trend + Recent Sales, Right Low Stock Table */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        {/* Left Pane (2 Columns) */}
        <div className="lg:col-span-2 flex flex-col gap-4">
          {/* 7-Day Sales vs Purchases Comparative Bar Ribbon */}
          <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">
                Sales vs Purchases 7-Day Activity
              </h2>
              <span className="text-[10px] text-slate-400 font-mono">Daily volume (₹)</span>
            </div>

            {trendData.length === 0 ? (
              <div className="py-8 text-center text-xs text-slate-400">
                No recent activity recorded
              </div>
            ) : (
              <div className="flex flex-col gap-2">
                {trendData.map((dp) => {
                  const sAmt = dp.salesAmount;
                  const pAmt = dp.purchaseAmount;
                  const maxAmt = Math.max(1000, ...trendData.map((t) => Math.max(t.salesAmount, t.purchaseAmount)));
                  const sPct = Math.min(100, (sAmt / maxAmt) * 100);
                  const pPct = Math.min(100, (pAmt / maxAmt) * 100);

                  return (
                    <div key={dp.date} className="flex items-center gap-3 text-xs">
                      <span className="w-20 font-mono text-slate-500 text-[11px] shrink-0">
                        {dp.date}
                      </span>
                      <div className="flex-1 flex flex-col gap-1">
                        <div className="flex items-center gap-2">
                          <div
                            style={{ width: `${Math.max(4, sPct)}%` }}
                            className="h-2 rounded bg-blue-600 transition-all duration-300"
                            title={`Sales: ₹${sAmt}`}
                          />
                          <span className="font-mono text-[10px] text-slate-600 dark:text-slate-400">
                            <CurrencyDisplay amount={sAmt} size="sm" />
                          </span>
                        </div>
                        {pAmt > 0 && (
                          <div className="flex items-center gap-2">
                            <div
                              style={{ width: `${Math.max(4, pPct)}%` }}
                              className="h-2 rounded bg-amber-500 transition-all duration-300"
                              title={`Purchases: ₹${pAmt}`}
                            />
                            <span className="font-mono text-[10px] text-amber-600 dark:text-amber-400">
                              <CurrencyDisplay amount={pAmt} size="sm" />
                            </span>
                          </div>
                        )}
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>

          {/* Recent Invoices Feed */}
          <div className="p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
            <div className="flex items-center justify-between mb-3">
              <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">
                Recent Invoices Feed
              </h2>
              <button
                type="button"
                onClick={() => setActiveRoute('sales-history')}
                className="text-xs font-medium text-blue-600 hover:underline flex items-center gap-1 cursor-pointer"
              >
                <span>View Full Log</span>
                <ArrowUpRight className="h-3 w-3" />
              </button>
            </div>

            {recentSales.length === 0 ? (
              <div className="py-6 text-center text-xs text-slate-400">No invoices yet today</div>
            ) : (
              <div className="overflow-x-auto">
                <table className="w-full text-left text-xs">
                  <thead>
                    <tr className="border-b border-slate-100 dark:border-slate-800 text-slate-400 select-none">
                      <th className="py-2">Code</th>
                      <th className="py-2">Customer</th>
                      <th className="py-2 text-right">Amount</th>
                      <th className="py-2 text-center">Status</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-slate-50 dark:divide-slate-800/60 font-mono">
                    {recentSales.map((s) => (
                      <tr key={s.id} className="hover:bg-slate-50/50">
                        <td className="py-2 font-semibold text-slate-800 dark:text-slate-200">
                          {s.transactionCode}
                        </td>
                        <td className="py-2 font-sans text-slate-600 dark:text-slate-400">
                          {(s.partyId !== null && customerNames[s.partyId]) || 'Walk-in Customer'}
                        </td>
                        <td className="py-2 text-right font-bold text-slate-900 dark:text-slate-100">
                          <CurrencyDisplay amount={s.totalAmount} size="sm" />
                        </td>
                        <td className="py-2 text-center">
                          <PaymentStatusBadge status={s.paymentStatus} />
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        </div>

        {/* Right Contextual Pane: Low Stock Watchlist */}
        <div className="flex flex-col p-4 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-1.5">
              <AlertTriangle className="h-4 w-4 text-amber-500" />
              <h2 className="text-xs font-bold uppercase tracking-wider text-slate-700 dark:text-slate-300">
                Low Stock Alerts
              </h2>
            </div>
            <span className="text-[10px] font-mono text-amber-600 bg-amber-50 dark:bg-amber-950/60 px-1.5 py-0.5 rounded font-bold">
              {lowStockAlerts.length}
            </span>
          </div>

          {lowStockAlerts.length === 0 ? (
            <div className="py-12 text-center text-xs text-slate-400">
              Inventory is healthy. Zero items under low-stock threshold.
            </div>
          ) : (
            <div className="flex flex-col divide-y divide-slate-100 dark:divide-slate-800 overflow-y-auto max-h-[460px]">
              {lowStockAlerts.map((item) => {
                const stock = item.currentStock;
                const unit = item.baseUnit;

                return (
                  <div key={item.id} className="py-2.5 flex items-center justify-between gap-2">
                    <div className="flex flex-col truncate">
                      <span className="text-xs font-semibold text-slate-800 dark:text-slate-200 truncate">
                        {item.name}
                      </span>
                      <span className="text-[10px] text-slate-400">{item.category}</span>
                    </div>

                    <div className="flex items-center gap-2 shrink-0">
                      <StockStatusBadge currentStock={stock} isLowStock baseUnit={unit} />
                      <button
                        type="button"
                        onClick={() => setActiveRoute('purchase')}
                        title="Record Inward Purchase Batch (F3)"
                        className="px-2 py-0.5 rounded text-[10px] font-semibold bg-blue-50 text-blue-700 dark:bg-blue-950 dark:text-blue-300 hover:bg-blue-100 transition-colors cursor-pointer"
                      >
                        Inward
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  );
};

export default DashboardPage;
