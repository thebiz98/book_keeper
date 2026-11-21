import React, { useState, useEffect } from 'react';
import { Truck, Search, Eye, DollarSign } from 'lucide-react';
import { api } from '@/api';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { PaymentStatusBadge } from '@/components/common/PaymentStatusBadge';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { SettlementModal } from '@/components/domain/SettlementModal';

export const PurchaseHistoryPage: React.FC = () => {
  const [purchases, setPurchases] = useState<any[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [searchQuery, setSearchQuery] = useState('');
  const [statusFilter, setStatusFilter] = useState<'ALL' | 'PAID' | 'PARTIAL' | 'UNPAID'>('ALL');
  const [datePreset, setDatePreset] = useState<'ALL' | 'TODAY' | '7D' | 'THIS_MONTH'>('ALL');

  // Purchase Details Modal
  const [selectedPurchase, setSelectedPurchase] = useState<any>(null);
  const [purchaseDetails, setPurchaseDetails] = useState<any>(null);
  const [isDetailsOpen, setIsDetailsOpen] = useState(false);

  // Settlement Modal State for paying vendor dues
  const [settlingVendor, setSettlingVendor] = useState<{ id: number; name: string; due: number } | null>(null);

  const loadPurchases = () => {
    setIsLoading(true);
    api
      .getPurchasesHistory()
      .then((res) => {
        const list = Array.isArray(res) ? res : res?.items || [];
        setPurchases(list);
      })
      .catch(() => setPurchases([]))
      .finally(() => setIsLoading(false));
  };

  useEffect(() => {
    loadPurchases();
  }, []);

  const openDetails = async (purchase: any) => {
    setSelectedPurchase(purchase);
    setIsDetailsOpen(true);
    const purchaseId = purchase.purchaseId ?? purchase.purchase_id ?? purchase.id;
    if (purchaseId && api.getPurchaseDetails) {
      try {
        const details = await api.getPurchaseDetails(purchaseId);
        setPurchaseDetails(details);
      } catch {
        setPurchaseDetails(null);
      }
    }
  };

  const filteredPurchases = purchases.filter((p) => {
    const code = (p.purchaseCode || p.purchase_code || p.transactionCode || '').toLowerCase();
    const vendor = (p.vendorName || p.vendor_name || p.partyName || p.party_name || '').toLowerCase();
    const supplierRef = (p.supplierInvoiceRef || p.supplier_invoice_ref || p.notes || '').toLowerCase();
    const matchesSearch =
      !searchQuery.trim() ||
      code.includes(searchQuery.toLowerCase()) ||
      vendor.includes(searchQuery.toLowerCase()) ||
      supplierRef.includes(searchQuery.toLowerCase());

    const status = (p.paymentStatus || p.payment_status || 'PAID').toUpperCase();
    const matchesStatus = statusFilter === 'ALL' || status === statusFilter;

    let matchesDate = true;
    const purchaseDateStr = p.date || p.transactionDate || p.inwardDate || p.created_at;
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
              filteredPurchases.map((purchase, idx) => {
                const code = purchase.purchaseCode || purchase.purchase_code || purchase.transactionCode || `PUR-${purchase.id || idx + 1}`;
                const vendor = purchase.vendorName || purchase.vendor_name || purchase.partyName || 'General Supplier';
                const total = purchase.totalAmount ?? purchase.total_amount ?? 0;
                const paid = purchase.paidAmount ?? purchase.paid_amount ?? total;
                const due = purchase.balanceDue ?? purchase.balance_due ?? 0;
                const status = purchase.paymentStatus || purchase.payment_status || (due === 0 ? 'PAID' : paid > 0 ? 'PARTIAL' : 'UNPAID');
                const date = purchase.date || purchase.transactionDate || purchase.inwardDate || 'Today';

                return (
                  <TableRow key={purchase.id || purchase.purchaseId || idx}>
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
                        {due > 0 && purchase.partyId && (
                          <Button
                            variant="secondary"
                            size="xs"
                            leftIcon={<DollarSign size={12} />}
                            onClick={() =>
                              setSettlingVendor({
                                id: purchase.partyId,
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
          title={`Purchase Details: ${selectedPurchase.purchaseCode || selectedPurchase.purchase_code || 'Inward Batch'}`}
          size="lg"
        >
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-2 text-xs bg-slate-50 dark:bg-slate-800 p-3 rounded-md">
              <div>
                <span className="text-slate-500">Supplier / Vendor: </span>
                <span className="font-medium text-slate-800 dark:text-slate-200">
                  {selectedPurchase.vendorName || selectedPurchase.vendor_name || 'General Supplier'}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Inward Date: </span>
                <span className="font-mono text-slate-800 dark:text-slate-200">
                  {selectedPurchase.date || selectedPurchase.transactionDate || 'Today'}
                </span>
              </div>
              <div>
                <span className="text-slate-500">Payment Status: </span>
                <PaymentStatusBadge status={selectedPurchase.paymentStatus || selectedPurchase.payment_status || 'PAID'} />
              </div>
              <div>
                <span className="text-slate-500">Purchase Code: </span>
                <span className="font-mono font-semibold text-blue-600 dark:text-blue-400">
                  {selectedPurchase.purchaseCode || selectedPurchase.purchase_code}
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
                  {((purchaseDetails?.items || selectedPurchase.items || []) as any[]).length === 0 ? (
                    <TableRow>
                      <TableCell colSpan={5} align="center" className="py-4 text-slate-500">
                        Inward line item details archived.
                      </TableCell>
                    </TableRow>
                  ) : (
                    ((purchaseDetails?.items || selectedPurchase.items || []) as any[]).map((it: any, i: number) => {
                      const qty = it.quantity ?? it.bundle_quantity ?? 1;
                      const cost = it.unitCost ?? it.unit_cost ?? 0;
                      const total = qty * cost;
                      return (
                        <TableRow key={i}>
                          <TableCell mono className="text-xs text-slate-400">{i + 1}</TableCell>
                          <TableCell className="font-medium">{it.itemName || it.item_name || it.name || `Item #${it.itemId || i + 1}`}</TableCell>
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
                <CurrencyDisplay amount={selectedPurchase.totalAmount ?? selectedPurchase.total_amount ?? 0} size="md" />
              </div>
              <div className="flex items-center gap-8">
                <span className="text-slate-500 font-medium">Paid to Vendor:</span>
                <CurrencyDisplay amount={selectedPurchase.paidAmount ?? selectedPurchase.paid_amount ?? 0} size="md" />
              </div>
              {(selectedPurchase.balanceDue ?? selectedPurchase.balance_due ?? 0) > 0 && (
                <div className="flex items-center gap-8 text-rose-600 dark:text-rose-400 font-semibold">
                  <span>Vendor Payable Due:</span>
                  <CurrencyDisplay amount={selectedPurchase.balanceDue ?? selectedPurchase.balance_due ?? 0} colored size="md" />
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
