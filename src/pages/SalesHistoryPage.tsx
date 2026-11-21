import React, { useState, useEffect } from 'react';
import { Receipt, Search, Eye, DollarSign } from 'lucide-react';
import { api } from '@/api';
import type { PaymentStatus, Sale, SaleDetailView } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { PaymentStatusBadge } from '@/components/common/PaymentStatusBadge';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { SettlementModal } from '@/components/domain/SettlementModal';

export const SalesHistoryPage: React.FC = () => {
  const [sales, setSales] = useState<Sale[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<'ALL' | PaymentStatus>('ALL');
  const [datePreset, setDatePreset] = useState<'ALL' | 'TODAY' | '7D' | 'THIS_MONTH'>('ALL');

  // Receipt Modal State
  const [selectedSale, setSelectedSale] = useState<Sale | null>(null);
  const [saleDetails, setSaleDetails] = useState<SaleDetailView | null>(null);
  const [customerNames, setCustomerNames] = useState<Record<number, string>>({});
  const [isReceiptOpen, setIsReceiptOpen] = useState(false);

  // Settlement Modal State
  const [settlingParty, setSettlingParty] = useState<{ id: number; name: string; due: number } | null>(null);

  const loadSales = () => {
    setIsLoading(true);
    api
      .getSalesHistory()
      .then((res) => setSales(res.items))
      .catch(() => setSales([]))
      .finally(() => setIsLoading(false));
    api
      .listParties('CUSTOMER', false)
      .then((rows) => setCustomerNames(Object.fromEntries(rows.map((c) => [c.id, c.name]))))
      .catch(() => setCustomerNames({}));
  };

  const customerOf = (s: Sale): string =>
    (s.partyId !== null && customerNames[s.partyId]) || 'Walk-in Customer';

  useEffect(() => {
    loadSales();
  }, []);

  const openReceipt = async (sale: Sale) => {
    setSelectedSale(sale);
    setSaleDetails(null);
    setIsReceiptOpen(true);
    try {
      setSaleDetails(await api.getSaleDetails(sale.id));
    } catch {
      setSaleDetails(null);
    }
  };

  const filteredSales = sales.filter((s) => {
    const code = s.transactionCode.toLowerCase();
    const customer = customerOf(s).toLowerCase();
    const matchesSearch = !searchQuery.trim() || code.includes(searchQuery.toLowerCase()) || customer.includes(searchQuery.toLowerCase());

    const matchesStatus = statusFilter === 'ALL' || s.paymentStatus === statusFilter;

    // Date filtering
    let matchesDate = true;
    const saleDateStr = s.transactionDate;
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
              filteredSales.map((sale) => {
                const code = sale.transactionCode;
                const customer = customerOf(sale);
                const total = sale.totalAmount;
                const paid = sale.paidAmount;
                const due = sale.balanceDue;
                const status = sale.paymentStatus;
                const partyId = sale.partyId;
                const date = sale.transactionDate;

                return (
                  <TableRow key={sale.id}>
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
                        {due > 0 && partyId !== null && (
                          <Button
                            variant="secondary"
                            size="xs"
                            leftIcon={<DollarSign size={12} />}
                            onClick={() =>
                              setSettlingParty({
                                id: partyId,
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
          title={`Receipt: ${selectedSale.transactionCode}`}
          size="lg"
        >
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-2 text-xs bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
              <div>
                <span className="text-slate-500">Customer: </span>
                <span className="font-medium text-slate-800 dark:text-slate-200">
                  {customerOf(selectedSale)}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Date: </span>
                <span className="font-mono text-slate-800 dark:text-slate-200">
                  {selectedSale.transactionDate}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Payment Status: </span>
                <PaymentStatusBadge status={selectedSale.paymentStatus} />
              </div>
              <div>
                <span className="text-slate-500">Invoice: </span>
                <span className="font-mono font-semibold text-blue-600 dark:text-blue-400">
                  {selectedSale.transactionCode}
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
                  {(saleDetails?.items ?? []).length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={5} align="center" className="py-4 text-slate-500">
                        Line items unavailable for historical record.
                      </TableCell>
                    </TableRow>
                  ) : (
                    (saleDetails?.items ?? []).map((it, i) => {
                      const qty = it.saleItem.bundleQuantity;
                      const total = it.saleItem.totalAmount;
                      const rate = qty > 0 ? total / qty : 0;
                      return (
                        <TableRow key={i}>
                          <TableCell mono className="text-xs text-slate-400">{i + 1}</TableCell>
                          <TableCell className="font-medium">{it.itemName}</TableCell>
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
                <CurrencyDisplay amount={selectedSale.totalAmount} size="md" />
              </div>
              <div className="flex items-center gap-8">
                <span className="text-slate-500 font-medium">Paid Amount:</span>
                <CurrencyDisplay amount={selectedSale.paidAmount} size="md" />
              </div>
              {(selectedSale.balanceDue) > 0 && (
                <div className="flex items-center gap-8 text-rose-600 dark:text-rose-400 font-semibold">
                  <span>Balance Due:</span>
                  <CurrencyDisplay amount={selectedSale.balanceDue} colored size="md" />
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
