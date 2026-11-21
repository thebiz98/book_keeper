import React, { useState, useEffect } from 'react';
import { RotateCcw, AlertTriangle, ArrowRightLeft, CheckCircle2 } from 'lucide-react';
import { api } from '@/api';
import type {
  ItemSearchResult,
  SettlementType,
  StockAdjustment,
  StockCondition,
} from '@/types';
import { useAppStore } from '@/store/useAppStore';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { TextInput } from '@/components/ui/TextInput';
import { NumericInput } from '@/components/ui/NumericInput';
import { Select } from '@/components/ui/Select';
import { Tabs } from '@/components/ui/Tabs';
import { ItemSearchTypeahead } from '@/components/domain/ItemSearchTypeahead';
import { StockAdjustmentModal } from '@/components/domain/StockAdjustmentModal';

export const ReturnsAdjustmentsPage: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'customer' | 'vendor' | 'adjustments'>('customer');
  const recordCashFlow = useAppStore((s) => s.recordCashFlow);

  // Success / Feedback Message
  const [feedback, setFeedback] = useState<{ type: 'success' | 'error'; message: string } | null>(null);

  // Tab 1: Customer Return State
  const [custReturnItem, setCustReturnItem] = useState<ItemSearchResult | null>(null);
  const [custReturnQty, setCustReturnQty] = useState(1);
  const [custCondition, setCustCondition] = useState<StockCondition>('Restockable');
  const [custResolution, setCustResolution] = useState<SettlementType>('CashRefund');
  const [custRefundAmount, setCustRefundAmount] = useState(0);
  const [custNotes, setCustNotes] = useState('');
  const [isSubmittingCust, setIsSubmittingCust] = useState(false);

  // Tab 2: Vendor Return State
  const [vendReturnItem, setVendReturnItem] = useState<ItemSearchResult | null>(null);
  const [vendReturnQty, setVendReturnQty] = useState(1);
  const [vendResolution, setVendResolution] = useState<SettlementType>('ReduceDue');
  const [vendRefundAmount, setVendRefundAmount] = useState(0);
  const [vendNotes, setVendNotes] = useState('');
  const [isSubmittingVend, setIsSubmittingVend] = useState(false);

  // Tab 3: Stock Adjustments State
  const [adjustments, setAdjustments] = useState<StockAdjustment[]>([]);
  const [isAdjustmentModalOpen, setIsAdjustmentModalOpen] = useState(false);

  const loadAdjustments = () => {
    api
      .listStockAdjustments()
      .then((res) => setAdjustments(res.items))
      .catch(() => setAdjustments([]));
  };

  useEffect(() => {
    loadAdjustments();
  }, []);

  const handleCustomerReturn = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!custReturnItem) {
      setFeedback({ type: 'error', message: 'Please select an item to return.' });
      return;
    }
    setIsSubmittingCust(true);
    setFeedback(null);
    try {
      const unitPrice =
        custResolution === 'CashRefund' && custReturnQty > 0
          ? custRefundAmount / custReturnQty
          : custReturnItem.defaultSalePrice;
      const returnCode = await api.createCustomerReturn({
        originalSaleId: null,
        partyId: null,
        returnDate: new Date().toISOString().split('T')[0],
        settlementType: custResolution,
        notes: custNotes.trim() || null,
        items: [
          {
            itemId: custReturnItem.itemId,
            saleItemId: null,
            quantity: custReturnQty,
            unitPrice,
            condition: custCondition,
          },
        ],
      });
      if (custResolution === 'CashRefund' && custRefundAmount > 0) {
        recordCashFlow({ refunds: custRefundAmount });
      }

      setFeedback({
        type: 'success',
        message: `Customer return recorded successfully (${returnCode}).`,
      });
      setCustReturnItem(null);
      setCustReturnQty(1);
      setCustRefundAmount(0);
      setCustNotes('');
    } catch (err: any) {
      setFeedback({ type: 'error', message: err?.message || 'Failed to record customer return.' });
    } finally {
      setIsSubmittingCust(false);
    }
  };

  const handleVendorReturn = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!vendReturnItem) {
      setFeedback({ type: 'error', message: 'Please select an item to return to vendor.' });
      return;
    }
    setIsSubmittingVend(true);
    setFeedback(null);
    try {
      const returnCode = await api.createVendorReturn({
        originalPurchaseId: null,
        partyId: null,
        returnDate: new Date().toISOString().split('T')[0],
        settlementType: vendResolution,
        reason: null,
        notes: vendNotes.trim() || null,
        items: [
          {
            itemId: vendReturnItem.itemId,
            purchaseItemId: null,
            quantity: vendReturnQty,
            unitCost: vendReturnQty > 0 ? vendRefundAmount / vendReturnQty : 0,
          },
        ],
      });
      setFeedback({
        type: 'success',
        message: `Vendor return recorded successfully (${returnCode}).`,
      });
      setVendReturnItem(null);
      setVendReturnQty(1);
      setVendRefundAmount(0);
      setVendNotes('');
    } catch (err: any) {
      setFeedback({ type: 'error', message: err?.message || 'Failed to record vendor return.' });
    } finally {
      setIsSubmittingVend(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        <div>
          <h1 className="text-xl font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
            <RotateCcw className="text-blue-600 dark:text-blue-400" size={24} />
            Returns & Stock Adjustments
          </h1>
          <p className="text-xs text-slate-500 mt-1">
            Manage customer returns, vendor returns, scrap write-offs, and stock count corrections.
          </p>
        </div>
        {activeTab === 'adjustments' && (
          <Button
            variant="primary"
            size="sm"
            leftIcon={<AlertTriangle size={14} />}
            onClick={() => setIsAdjustmentModalOpen(true)}
          >
            New Adjustment
          </Button>
        )}
      </div>

      {/* Notification banner */}
      {feedback && (
        <div
          className={`p-3 rounded-lg text-sm flex items-center gap-2 ${
            feedback.type === 'success'
              ? 'bg-emerald-50 dark:bg-emerald-950/40 text-emerald-800 dark:text-emerald-300 border border-emerald-200 dark:border-emerald-800'
              : 'bg-rose-50 dark:bg-rose-950/40 text-rose-800 dark:text-rose-300 border border-rose-200 dark:border-rose-800'
          }`}
        >
          {feedback.type === 'success' ? <CheckCircle2 size={16} /> : <AlertTriangle size={16} />}
          <span>{feedback.message}</span>
        </div>
      )}

      {/* Primary Tabs */}
      <Tabs
        tabs={[
          { id: 'customer', label: 'Customer Sales Return & Swap' },
          { id: 'vendor', label: 'Vendor Purchase Return' },
          { id: 'adjustments', label: 'Stock Adjustments & Scrap' },
        ]}
        activeTab={activeTab}
        onChange={(id) => {
          setActiveTab(id as any);
          setFeedback(null);
        }}
      />

      {/* Tab 1: Customer Sales Return */}
      {activeTab === 'customer' && (
        <div className="bg-white dark:bg-slate-900 p-6 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs max-w-2xl">
          <form onSubmit={handleCustomerReturn} className="space-y-4">
            <h2 className="text-base font-semibold text-slate-800 dark:text-slate-200">
              Record Customer Sales Return
            </h2>

            <div>
              <label className="text-xs font-medium text-slate-700 dark:text-slate-300 mb-1 block">
                Select Returned Item
              </label>
              {custReturnItem ? (
                <div className="flex items-center justify-between p-2.5 bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800 rounded-md">
                  <div>
                    <span className="font-semibold text-sm text-blue-900 dark:text-blue-200">
                      {custReturnItem.name}
                    </span>
                    <span className="text-xs text-slate-500 ml-2">
                      (Stock: {custReturnItem.currentStock})
                    </span>
                  </div>
                  <Button variant="ghost" size="xs" onClick={() => setCustReturnItem(null)}>
                    Change
                  </Button>
                </div>
              ) : (
                <ItemSearchTypeahead
                  onSelectItem={(item) => {
                    setCustReturnItem(item);
                    setCustRefundAmount(item.defaultSalePrice);
                  }}
                  placeholder="Search item to return..."
                />
              )}
            </div>

            <div className="grid grid-cols-2 gap-4">
              <NumericInput
                label="Return Quantity"
                value={custReturnQty}
                onChange={(v) => {
                  setCustReturnQty(v);
                  if (custReturnItem) {
                    const price = custReturnItem.defaultSalePrice;
                    setCustRefundAmount(v * price);
                  }
                }}
                min={1}
              />
              <Select
                label="Item Condition"
                value={custCondition}
                onChange={(v) => setCustCondition(v as StockCondition)}
                options={[
                  { value: 'Restockable', label: 'Restockable (Back to shelf)' },
                  { value: 'DamagedScrap', label: 'Damaged / Write-off to Scrap' },
                ]}
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <Select
                label="Settlement Resolution"
                value={custResolution}
                onChange={(v) => setCustResolution(v as SettlementType)}
                options={[
                  { value: 'CashRefund', label: 'Immediate Cash Refund' },
                  { value: 'ReduceDue', label: 'Reduce Customer Due Balance' },
                  { value: 'DefectiveReplacement', label: 'Direct Item Swap (Zero Cash)' },
                ]}
              />
              {custResolution === 'CashRefund' ? (
                <NumericInput
                  label="Refund Cash (₹)"
                  value={custRefundAmount}
                  onChange={setCustRefundAmount}
                  allowDecimals
                  min={0}
                />
              ) : (
                <div />
              )}
            </div>

            <TextInput
              label="Return Reason / Remarks"
              placeholder="e.g. Defective nib, incorrect size chosen..."
              value={custNotes}
              onChange={(e) => setCustNotes(e.target.value)}
            />

            <Button
              type="submit"
              variant="primary"
              isLoading={isSubmittingCust}
              leftIcon={<RotateCcw size={14} />}
            >
              Commit Customer Return
            </Button>
          </form>
        </div>
      )}

      {/* Tab 2: Vendor Purchase Return */}
      {activeTab === 'vendor' && (
        <div className="bg-white dark:bg-slate-900 p-6 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs max-w-2xl">
          <form onSubmit={handleVendorReturn} className="space-y-4">
            <h2 className="text-base font-semibold text-slate-800 dark:text-slate-200">
              Record Inward Return to Supplier / Vendor
            </h2>

            <div>
              <label className="text-xs font-medium text-slate-700 dark:text-slate-300 mb-1 block">
                Select Item to Return
              </label>
              {vendReturnItem ? (
                <div className="flex items-center justify-between p-2.5 bg-blue-50 dark:bg-blue-950/30 border border-blue-200 dark:border-blue-800 rounded-md">
                  <div>
                    <span className="font-semibold text-sm text-blue-900 dark:text-blue-200">
                      {vendReturnItem.name}
                    </span>
                    <span className="text-xs text-slate-500 ml-2">
                      (Stock: {vendReturnItem.currentStock})
                    </span>
                  </div>
                  <Button variant="ghost" size="xs" onClick={() => setVendReturnItem(null)}>
                    Change
                  </Button>
                </div>
              ) : (
                <ItemSearchTypeahead
                  onSelectItem={(item) => {
                    setVendReturnItem(item);
                    setVendRefundAmount(item.defaultSalePrice);
                  }}
                  placeholder="Search item to return to vendor..."
                />
              )}
            </div>

            <div className="grid grid-cols-2 gap-4">
              <NumericInput
                label="Return Quantity"
                value={vendReturnQty}
                onChange={(v) => {
                  setVendReturnQty(v);
                  if (vendReturnItem) {
                    setVendRefundAmount(v * vendReturnItem.defaultSalePrice);
                  }
                }}
                min={1}
              />
              <Select
                label="Vendor Resolution"
                value={vendResolution}
                onChange={(v) => setVendResolution(v as SettlementType)}
                options={[
                  { value: 'ReduceDue', label: 'Credit Note / Reduce Vendor Due' },
                  { value: 'CashRefund', label: 'Cash Refund Received from Vendor' },
                ]}
              />
            </div>

            <TextInput
              label="Reason / Notes"
              placeholder="e.g. Expired stock returned to distributor..."
              value={vendNotes}
              onChange={(e) => setVendNotes(e.target.value)}
            />

            <Button
              type="submit"
              variant="primary"
              isLoading={isSubmittingVend}
              leftIcon={<ArrowRightLeft size={14} />}
            >
              Commit Vendor Return
            </Button>
          </form>
        </div>
      )}

      {/* Tab 3: Stock Adjustments */}
      {activeTab === 'adjustments' && (
        <div className="space-y-4">
          <div className="bg-white dark:bg-slate-900 rounded-lg border border-slate-200 dark:border-slate-800 overflow-hidden shadow-xs">
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>#</TableHead>
                  <TableHead>Item</TableHead>
                  <TableHead align="right">Qty Delta</TableHead>
                  <TableHead>Reason</TableHead>
                  <TableHead>Date</TableHead>
                  <TableHead>Notes</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {adjustments.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={6} align="center" className="py-8 text-slate-500">
                      No stock adjustments recorded yet. Click "New Adjustment" above to log damage or inventory audit correction.
                    </TableCell>
                  </TableRow>
                ) : (
                  adjustments.map((adj, idx) => {
                    const delta = adj.quantity;
                    return (
                      <TableRow key={adj.id}>
                        <TableCell mono className="text-xs text-slate-400">{idx + 1}</TableCell>
                        <TableCell className="font-medium">{`Item #${adj.itemId}`}</TableCell>
                        <TableCell align="right" mono className={`font-semibold ${delta > 0 ? 'text-emerald-600' : 'text-rose-600'}`}>
                          {delta > 0 ? `+${delta}` : delta}
                        </TableCell>
                        <TableCell className="capitalize text-xs font-semibold">{adj.reason}</TableCell>
                        <TableCell className="text-xs text-slate-500">{adj.adjustmentDate}</TableCell>
                        <TableCell className="text-xs text-slate-500">{adj.notes || '—'}</TableCell>
                      </TableRow>
                    );
                  })
                )}
              </TableBody>
            </Table>
          </div>
        </div>
      )}

      {/* Stock Adjustment Modal */}
      <StockAdjustmentModal
        isOpen={isAdjustmentModalOpen}
        onClose={() => setIsAdjustmentModalOpen(false)}
        onAdjusted={() => {
          setIsAdjustmentModalOpen(false);
          loadAdjustments();
          setFeedback({ type: 'success', message: 'Stock adjustment saved successfully.' });
        }}
      />
    </div>
  );
};

export default ReturnsAdjustmentsPage;
