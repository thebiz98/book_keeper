import React, { useEffect, useState } from 'react';
import { Undo2 } from 'lucide-react';
import { Tabs } from '@/components/ui/Tabs';
import { ReturnFormCustomer } from '@/components/domain/ReturnFormCustomer';
import { ReturnFormVendor } from '@/components/domain/ReturnFormVendor';
import { StockAdjustmentForm } from '@/components/domain/StockAdjustmentForm';
import { AdjustmentsHistoryList, ReturnsHistoryList } from '@/components/domain/ReturnsHistoryList';
import { consumeNavParam, peekNavParam, parseReturnBill, refreshAppMetrics } from '@/components/domain/navParams';

type TabId = 'customer' | 'vendor' | 'adjust';

export const ReturnsAdjustmentsPage: React.FC = () => {
  // Pre-linked bill from Sales/Purchase History: query param `returnBill=SALE:<id>|PURCHASE:<id>`.
  const [linked] = useState(() => parseReturnBill(peekNavParam('returnBill')));
  const [tab, setTab] = useState<TabId>(linked?.kind === 'PURCHASE' ? 'vendor' : 'customer');
  const [reloadKey, setReloadKey] = useState(0);
  const bump = () => setReloadKey((k) => k + 1);

  useEffect(() => {
    consumeNavParam('returnBill'); // one-shot: clear after the state above captured it
  }, []);

  return (
    <div className="flex flex-col gap-4">
      <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        <h1 className="text-xl font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
          <Undo2 className="text-blue-600 dark:text-blue-400" size={24} />
          Returns, Replacements &amp; Adjustments
        </h1>
      </div>

      <Tabs
        activeTab={tab}
        onChange={(id) => setTab(id === 'vendor' ? 'vendor' : id === 'adjust' ? 'adjust' : 'customer')}
        tabs={[
          { id: 'customer', label: 'Customer Return & Swap' },
          { id: 'vendor', label: 'Vendor Return' },
          { id: 'adjust', label: 'Stock Adjustments' },
        ]}
      />

      <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
        {tab === 'customer' && (
          <ReturnFormCustomer initialBillId={linked?.kind === 'SALE' ? linked.id : null} onDone={bump} />
        )}
        {tab === 'vendor' && (
          <ReturnFormVendor initialBillId={linked?.kind === 'PURCHASE' ? linked.id : null} onDone={bump} />
        )}
        {tab === 'adjust' && (
          <div className="max-w-xl">
            <StockAdjustmentForm
              onAdjusted={() => {
                refreshAppMetrics();
                bump();
              }}
            />
          </div>
        )}
      </div>

      <div>
        <h2 className="text-xs font-bold uppercase tracking-wider text-slate-600 dark:text-slate-300 mb-2">
          {tab === 'adjust' ? 'Recent stock adjustments' : 'Recent returns'}
        </h2>
        {tab === 'adjust' ? (
          <AdjustmentsHistoryList reloadKey={reloadKey} />
        ) : (
          <ReturnsHistoryList returnType={tab === 'vendor' ? 'VENDOR' : 'CUSTOMER'} reloadKey={reloadKey} />
        )}
      </div>
    </div>
  );
};

export default ReturnsAdjustmentsPage;
