import React, { useState, useEffect } from 'react';
import { Truck, Search, Eye, DollarSign } from 'lucide-react';
import { api } from '@/api';
import type { PaymentStatus, Purchase, PurchaseDetailView } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { PaymentStatusBadge } from '@/components/common/PaymentStatusBadge';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { SettlementModal } from '@/components/domain/SettlementModal';

export const PurchaseHistoryPage: React.FC = () => {
  const [purchases, setPurchases] = useState<Purchase[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<'ALL' | PaymentStatus>('ALL');
  const [datePreset, setDatePreset] = useState<'ALL' | 'TODAY' | '7D' | 'THIS_MONTH'>('ALL');

  // Purchase Details Modal
  const [selectedPurchase, setSelectedPurchase] = useState<Purchase | null>(null);
  const [purchaseDetails, setPurchaseDetails] = useState<PurchaseDetailView | null>(null);
  const [vendorNames, setVendorNames] = useState<Record<number, string>>({});
  const [isDetailsOpen, setIsDetailsOpen] = useState(false);

  // Settlement Modal State for paying vendor dues
  const [settlingVendor, setSettlingVendor] = useState<{ id: number; name: string; due: number } | null>(null);

  const loadPurchases = () => {
    setIsLoading(true);
    api
      .getPurchasesHistory()
      .then((res) => setPurchases(res.items))
      .catch(() => setPurchases([]))
      .finally(() => setIsLoading(false));
    api
      .listParties('VENDOR', false)
      .then((rows) => setVendorNames(Object.fromEntries(rows.map((v) => [v.id, v.name]))))
      .catch(() => setVendorNames({}));
  };

  const vendorOf = (p: Purchase): string =>
    (p.partyId !== null && vendorNames[p.partyId]) || 'General Supplier';

  useEffect(() => {
    loadPurchases();
  }, []);

  const openDetails = async (purchase: Purchase) => {
    setSelectedPurchase(purchase);
    setPurchaseDetails(null);
    setIsDetailsOpen(true);
    try {
      setPurchaseDetails(await api.getPurchaseDetails(purchase.id));
    } catch {
      setPurchaseDetails(null);
    }
  };

  const filteredPurchases = purchases.filter((p) => {
    const code = p.transactionCode.toLowerCase();
    const vendor = vendorOf(p).toLowerCase();
    const supplierRef = (p.notes ?? '').toLowerCase();
    const matchesSearch =
      !searchQuery.trim() ||
      code.includes(searchQuery.toLowerCase()) ||
      vendor.includes(searchQuery.toLowerCase()) ||
      supplierRef.includes(searchQuery.toLowerCase());

    const matchesStatus = statusFilter === 'ALL' || p.paymentStatus === statusFilter;

    let matchesDate = true;
    const purchaseDateStr = p.transactionDate;
    if (purchaseDateStr && datePreset !== 'ALL') {
      const pDate = new Date(purchaseDateStr);
      const now = new Date();
      if (datePreset === 'TODAY') {
        matchesDate = pDate.toDateString() === now.toDateString();
      } else if (datePreset === '7D') {
        const diffDays = (now.getTime() - pDate.getTime()) / (1000 * 3600 * 24);
        matchesDate = diffDays <= 7;
      } else if (datePreset === 'THIS_MONTH') {
        matchesDate = pDate.getMonth() === now.getMonth() && pDate.getFullYear() === now.getFullYear();
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
            <Truck className="text-blue-600 dark:text-blue-400" size={24} />
            Inward Purchases & Lot History
          </h1>
          <p className="text-xs text-slate-500 mt-1">
            Audit inward supplier purchases, batch lots, invoices, and supplier payable balances.
          </p>
        </div>
      </div>

      {/* Filter Toolbar */}
      <div className="flex flex-wrap items-center gap-3 bg-white dark:bg-slate-900 p-3 rounded-lg border border-slate-200 dark:border-slate-800">
        <div className="relative flex-1 min-w-[200px]">
          <Search className="absolute left-2.5 top-2.5 text-slate-400" size={16} />
          <input
            type="text"
            placeholder="Search purchase code, vendor, or supplier ref..."
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

      {/* Purchases Audit Table */}
      <div className="bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-800 overflow-hidden shadow-xs">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Purchase Code</TableHead>
              <TableHead>Inward Date</TableHead>
              <TableHead>Vendor / Supplier</TableHead>
              <TableHead align="right">Total Value (₹)</TableHead>
              <TableHead align="right">Paid (₹)</TableHead>
              <TableHead align="right">Payable Due (₹)</TableHead>
              <TableHead align="center">Status</TableHead>
              <TableHead align="right">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={8} align="center" className="py-8 text-slate-500">
                  Loading purchase history...
                </TableCell>
              </TableRow>
            ) : filteredPurchases.length === 0 ? (
              <TableRow>
                <TableCell colSpan={8} align="center" className="py-8 text-slate-500">
                  No inward purchase records found matching filters.
                </TableCell>
              </TableRow>
            ) : (
              filteredPurchases.map((purchase) => {
                const code = purchase.transactionCode;
                const vendor = vendorOf(purchase);
                const total = purchase.totalAmount;
                const paid = purchase.paidAmount;
                const due = purchase.balanceDue;
                const status = purchase.paymentStatus;
                const partyId = purchase.partyId;
                const date = purchase.transactionDate;

                return (
                  <TableRow key={purchase.id}>
                    <TableCell mono className="font-semibold text-blue-600 dark:text-blue-400">
                      {code}
                    </TableCell>
                    <TableCell className="text-xs text-slate-500">{date}</TableCell>
                    <TableCell className="font-medium">{vendor}</TableCell>
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
                          onClick={() => openDetails(purchase)}
                        >
                          View
                        </Button>
                        {due > 0 && partyId !== null && (
                          <Button
                            variant="secondary"
                            size="xs"
                            leftIcon={<DollarSign size={12} />}
                            onClick={() =>
                              setSettlingVendor({
                                id: partyId,
                                name: vendor,
                                due,
                              })
                            }
                          >
                            Pay Due
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

      {/* Batch Details Modal */}
      {selectedPurchase && (
        <Modal
          isOpen={isDetailsOpen}
          onClose={() => setIsDetailsOpen(false)}
          title={`Purchase Details: ${selectedPurchase.transactionCode}`}
          size="lg"
        >
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-2 text-xs bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
              <div>
                <span className="text-slate-500">Supplier / Vendor: </span>
                <span className="font-medium text-slate-800 dark:text-slate-200">
                  {vendorOf(selectedPurchase)}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Inward Date: </span>
                <span className="font-mono text-slate-800 dark:text-slate-200">
                  {selectedPurchase.transactionDate}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Payment Status: </span>
                <PaymentStatusBadge status={selectedPurchase.paymentStatus} />
              </div>
              <div>
                <span className="text-slate-500">Purchase Code: </span>
                <span className="font-mono font-semibold text-blue-600 dark:text-blue-400">
                  {selectedPurchase.transactionCode}
                </span>
              </div>
            </div>

            {/* Inward Items Table */}
            <div className="border border-slate-200 dark:border-slate-800 rounded-md overflow-hidden">
              <Table dense>
                <TableHeader>
                  <TableRow>
                    <TableHead>#</TableHead>
                    <TableHead>Item</TableHead>
                    <TableHead align="right">Inward Qty</TableHead>
                    <TableHead align="right">Unit Cost (₹)</TableHead>
                    <TableHead align="right">Line Cost (₹)</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {(purchaseDetails?.items ?? []).length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={5} align="center" className="py-4 text-slate-500">
                        Inward line item details archived.
                      </TableCell>
                    </TableRow>
                  ) : (
                    (purchaseDetails?.items ?? []).map((it, i) => {
                      const qty = it.purchaseItem.bundleQuantity;
                      const total = it.purchaseItem.totalAmount;
                      const cost = qty > 0 ? total / qty : 0;
                      return (
                        <TableRow key={i}>
                          <TableCell mono className="text-xs text-slate-400">{i + 1}</TableCell>
                          <TableCell className="font-medium">{it.itemName}</TableCell>
                          <TableCell align="right" mono>{qty}</TableCell>
                          <TableCell align="right" mono>
                            <CurrencyDisplay amount={cost} />
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

            {/* Totals Summary */}
            <div className="flex flex-col items-end gap-1 pt-2 border-t border-slate-200 dark:border-slate-800 text-sm">
              <div className="flex items-center gap-8">
                <span className="text-slate-500 font-medium">Total Inward Cost:</span>
                <CurrencyDisplay amount={selectedPurchase.totalAmount} size="md" />
              </div>
              <div className="flex items-center gap-8">
                <span className="text-slate-500 font-medium">Paid to Vendor:</span>
                <CurrencyDisplay amount={selectedPurchase.paidAmount} size="md" />
              </div>
              {(selectedPurchase.balanceDue) > 0 && (
                <div className="flex items-center gap-8 text-rose-600 dark:text-rose-400 font-semibold">
                  <span>Vendor Payable Due:</span>
                  <CurrencyDisplay amount={selectedPurchase.balanceDue} colored size="md" />
                </div>
              )}
            </div>
          </div>
        </Modal>
      )}

      {/* Settle Vendor Due Modal */}
      {settlingVendor && (
        <SettlementModal
          isOpen={Boolean(settlingVendor)}
          onClose={() => setSettlingVendor(null)}
          partyId={settlingVendor.id}
          partyName={settlingVendor.name}
          totalDue={settlingVendor.due}
          onSettled={() => {
            setSettlingVendor(null);
            loadPurchases();
          }}
        />
      )}
    </div>
  );
};

export default PurchaseHistoryPage;
