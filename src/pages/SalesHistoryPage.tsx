import React, { useState, useEffect } from 'react';
import { Receipt, Search, Eye, DollarSign } from 'lucide-react';
import { api } from '@/api';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { PaymentStatusBadge } from '@/components/common/PaymentStatusBadge';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { SettlementModal } from '@/components/domain/SettlementModal';

export const SalesHistoryPage: React.FC = () => {
  const [sales, setSales] = useState<any[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<'ALL' | 'PAID' | 'PARTIAL' | 'UNPAID'>('ALL');
  const [datePreset, setDatePreset] = useState<'ALL' | 'TODAY' | '7D' | 'THIS_MONTH'>('ALL');

  // Receipt Modal State
  const [selectedSale, setSelectedSale] = useState<any>(null);
  const [saleDetails, setSaleDetails] = useState<any>(null);
  const [isReceiptOpen, setIsReceiptOpen] = useState(false);

  // Settlement Modal State
  const [settlingParty, setSettlingParty] = useState<{ id: number; name: string; due: number } | null>(null);

  const loadSales = () => {
    setIsLoading(true);
    api
      .getSalesHistory()
      .then((res) => {
        const list = Array.isArray(res) ? res : res?.items || [];
        setSales(list);
      })
      .catch(() => setSales([]))
      .finally(() => setIsLoading(false));
  };

  useEffect(() => {
    loadSales();
  }, []);

  const openReceipt = async (sale: any) => {
    setSelectedSale(sale);
    setIsReceiptOpen(true);
    const saleId = sale.saleId ?? sale.sale_id ?? sale.id;
    if (saleId && api.getSaleDetails) {
      try {
        const details = await api.getSaleDetails(saleId);
        setSaleDetails(details);
      } catch {
        setSaleDetails(null);
      }
    }
  };

  const filteredSales = sales.filter((s) => {
    const code = (s.saleCode || s.sale_code || s.transactionCode || s.transaction_code || '').toLowerCase();
    const customer = (s.customerName || s.customer_name || s.partyName || s.party_name || 'Walk-in Customer').toLowerCase();
    const matchesSearch = !searchQuery.trim() || code.includes(searchQuery.toLowerCase()) || customer.includes(searchQuery.toLowerCase());

    const status = (s.paymentStatus || s.payment_status || 'PAID').toUpperCase();
    const matchesStatus = statusFilter === 'ALL' || status === statusFilter;

    // Date filtering
    let matchesDate = true;
    const saleDateStr = s.date || s.transactionDate || s.transaction_date || s.created_at;
    if (saleDateStr && datePreset !== 'ALL') {
      const saleDate = new Date(saleDateStr);
      const now = new Date();
      if (datePreset === 'TODAY') {
        matchesDate = saleDate.toDateString() === now.toDateString();
      } else if (datePreset === '7D') {
        const diffDays = (now.getTime() - saleDate.getTime()) / (1000 * 3600 * 24);
        matchesDate = diffDays <= 7;
      } else if (datePreset === 'THIS_MONTH') {
        matchesDate = saleDate.getMonth() === now.getMonth() && saleDate.getFullYear() === now.getFullYear();
      }
    }

    return matchesSearch && matchesStatus && matchesDate;
  });

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        <div>
          <h1 className="text-xl font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
            <Receipt className="text-blue-600 dark:text-blue-400" size={24} />
            Sales History & Audit Log
          </h1>
          <p className="text-xs text-slate-500 mt-1">
            Browse and inspect all counter sales invoices, payment receipts, and balance dues.
          </p>
        </div>
      </div>

      {/* Filter Toolbar */}
      <div className="flex flex-wrap items-center gap-3 bg-white dark:bg-slate-900 p-3 rounded-lg border border-slate-200 dark:border-slate-800">
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-2.5 top-2.5 text-slate-400" size={16} />
          <input
            type="text"
            placeholder="Search invoice code or customer..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full pl-8 pr-3 py-1.5 text-sm bg-slate-50 dark:bg-slate-800 border border-slate-300 dark:border-slate-700 rounded-md focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        {/* Status Filter */}
        <div className="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 p-1 rounded-md text-xs font-medium">
          {(['ALL', 'PAID', 'PARTIAL', 'UNPAID'] as const).map((st) => (
            <button
              key={st}
              onClick={() => setStatusFilter(st)}
              className={`px-2.5 py-1 rounded transition-colors ${
                statusFilter === st
                  ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 font-semibold shadow-2xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
              }`}
            >
              {st}
            </button>
          ))}
        </div>

        {/* Date Filter */}
        <div className="flex items-center gap-1 bg-slate-100 dark:bg-slate-800 p-1 rounded-md text-xs font-medium">
          {(['ALL', 'TODAY', '7D', 'THIS_MONTH'] as const).map((dp) => (
            <button
              key={dp}
              onClick={() => setDatePreset(dp)}
              className={`px-2.5 py-1 rounded transition-colors ${
                datePreset === dp
                  ? 'bg-white dark:bg-slate-900 text-blue-600 dark:text-blue-400 font-semibold shadow-2xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
              }`}
            >
              {dp === '7D' ? '7 Days' : dp === 'THIS_MONTH' ? 'This Month' : dp}
            </button>
          ))}
        </div>
      </div>

      {/* Sales Audit Table */}
      <div className="bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-800 overflow-hidden shadow-xs">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Invoice Code</TableHead>
              <TableHead>Date & Time</TableHead>
              <TableHead>Customer</TableHead>
              <TableHead align="right">Total (₹)</TableHead>
              <TableHead align="right">Paid (₹)</TableHead>
              <TableHead align="right">Due (₹)</TableHead>
              <TableHead align="center">Status</TableHead>
              <TableHead align="right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={8} align="center" className="py-8 text-slate-500">
                  Loading sales history...
                </TableCell>
              </TableRow>
            ) : filteredSales.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} align="center" className="py-8 text-slate-500">
                  No sales invoices found matching filters.
                </TableCell>
              </TableRow>
            ) : (
              filteredSales.map((sale, idx) => {
                const code = sale.saleCode || sale.sale_code || sale.transactionCode || `SAL-${sale.id || idx + 1}`;
                const customer = sale.customerName || sale.customer_name || sale.partyName || 'Walk-in Customer';
                const total = sale.totalAmount ?? sale.total_amount ?? 0;
                const paid = sale.paidAmount ?? sale.paid_amount ?? total;
                const due = sale.balanceDue ?? sale.balance_due ?? 0;
                const status = sale.paymentStatus || sale.payment_status || (due === 0 ? 'PAID' : paid > 0 ? 'PARTIAL' : 'UNPAID');
                const date = sale.date || sale.transactionDate || sale.created_at || 'Today';

                return (
                  <TableRow key={sale.id || sale.saleId || idx}>
                    <TableCell mono className="font-semibold text-blue-600 dark:text-blue-400">
                      {code}
                    </TableCell>
                    <TableCell className="text-xs text-slate-500">{date}</TableCell>
                    <TableCell className="font-medium">{customer}</TableCell>
                    <TableCell align="right">
                      <CurrencyDisplay amount={total} />
                    </TableCell>
                    <TableCell align="right">
                      <CurrencyDisplay amount={paid} />
                    </TableCell>
                    <TableCell align="right">
                      <CurrencyDisplay amount={due} colored />
                    </TableCell>
                    <TableCell align="center">
                      <PaymentStatusBadge status={status} />
                    </TableCell>
                    <TableCell align="right">
                      <div className="flex items-center justify-end gap-1.5">
                        <Button
                          variant="ghost"
                          size="xs"
                          leftIcon={<Eye size={12} />}
                          onClick={() => openReceipt(sale)}
                        >
                          View
                        </Button>
                        {due > 0 && sale.customerId && (
                          <Button
                            variant="secondary"
                            size="xs"
                            leftIcon={<DollarSign size={12} />}
                            onClick={() =>
                              setSettlingParty({
                                id: sale.customerId,
                                name: customer,
                                due,
                              })
                            }
                          >
                            Settle
                          </Button>
                        )}
                      </div>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      {/* Receipt Details Modal */}
      {selectedSale && (
        <Modal
          isOpen={isReceiptOpen}
          onClose={() => setIsReceiptOpen(false)}
          title={`Receipt: ${selectedSale.saleCode || selectedSale.sale_code || selectedSale.transactionCode || 'Sale'}`}
          size="lg"
        >
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-2 text-xs bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
              <div>
                <span className="text-slate-500">Customer: </span>
                <span className="font-medium text-slate-800 dark:text-slate-200">
                  {selectedSale.customerName || selectedSale.customer_name || 'Walk-in Customer'}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Date: </span>
                <span className="font-mono text-slate-800 dark:text-slate-200">
                  {selectedSale.date || selectedSale.transactionDate || 'Today'}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Payment Status: </span>
                <PaymentStatusBadge status={selectedSale.paymentStatus || selectedSale.payment_status || 'PAID'} />
              </div>
              <div>
                <span className="text-slate-500">Invoice: </span>
                <span className="font-mono font-semibold text-blue-600 dark:text-blue-400">
                  {selectedSale.saleCode || selectedSale.sale_code || selectedSale.transactionCode}
                </span>
              </div>
            </div>

            {/* Line Items */}
            <div className="border border-slate-200 dark:border-slate-800 rounded-md overflow-hidden">
              <Table dense>
                <TableHeader>
                  <TableRow>
                    <TableHead>#</TableHead>
                    <TableHead>Item</TableHead>
                    <TableHead align="right">Qty</TableHead>
                    <TableHead align="right">Rate (₹)</TableHead>
                    <TableHead align="right">Line Total (₹)</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {((saleDetails?.items || selectedSale.items || []) as any[]).length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={5} align="center" className="py-4 text-slate-500">
                        Line items unavailable for historical record.
                      </TableCell>
                    </TableRow>
                  ) : (
                    ((saleDetails?.items || selectedSale.items || []) as any[]).map((it: any, i: number) => {
                      const qty = it.quantity ?? it.bundle_quantity ?? 1;
                      const rate = it.rate ?? it.unit_price ?? 0;
                      const total = qty * rate;
                      return (
                        <TableRow key={i}>
                          <TableCell mono className="text-xs text-slate-400">{i + 1}</TableCell>
                          <TableCell className="font-medium">{it.itemName || it.item_name || it.name || `Item #${it.itemId || i + 1}`}</TableCell>
                          <TableCell align="right" mono>{qty}</TableCell>
                          <TableCell align="right" mono>
                            <CurrencyDisplay amount={rate} />
                          </TableCell>
                          <TableCell align="right" mono className="font-semibold">
                            <CurrencyDisplay amount={total} />
                          </TableCell>
                        </TableRow>
                      );
                    })
                  )}
                </TableBody>
              </Table>
            </div>

            {/* Receipt Totals Summary */}
            <div className="flex flex-col items-end gap-1 pt-2 border-t border-slate-200 dark:border-slate-800 text-sm">
              <div className="flex items-center gap-8">
                <span className="text-slate-500 font-medium">Grand Total:</span>
                <CurrencyDisplay amount={selectedSale.totalAmount ?? selectedSale.total_amount ?? 0} size="md" />
              </div>
              <div className="flex items-center gap-8">
                <span className="text-slate-500 font-medium">Paid Amount:</span>
                <CurrencyDisplay amount={selectedSale.paidAmount ?? selectedSale.paid_amount ?? 0} size="md" />
              </div>
              {(selectedSale.balanceDue ?? selectedSale.balance_due ?? 0) > 0 && (
                <div className="flex items-center gap-8 text-rose-600 dark:text-rose-400 font-semibold">
                  <span>Balance Due:</span>
                  <CurrencyDisplay amount={selectedSale.balanceDue ?? selectedSale.balance_due ?? 0} colored size="md" />
                </div>
              )}
              {(selectedSale.changeAmount ?? selectedSale.changeReturned ?? 0) > 0 && (
                <div className="flex items-center gap-8 text-emerald-600 dark:text-emerald-400 font-semibold">
                  <span>Change Returned:</span>
                  <CurrencyDisplay amount={selectedSale.changeAmount ?? selectedSale.changeReturned ?? 0} size="md" />
                </div>
              )}
            </div>
          </div>
        </Modal>
      )}

      {/* Settle Due Modal */}
      {settlingParty && (
        <SettlementModal
          isOpen={Boolean(settlingParty)}
          onClose={() => setSettlingParty(null)}
          partyId={settlingParty.id}
          partyName={settlingParty.name}
          totalDue={settlingParty.due}
          onSettled={() => {
            setSettlingParty(null);
            loadSales();
          }}
        />
      )}
    </div>
  );
};

export default SalesHistoryPage;
