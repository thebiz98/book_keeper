import React from 'react';
import { Receipt } from 'lucide-react';
import { BillHistoryView } from '@/components/domain/BillHistoryView';

export const SalesHistoryPage: React.FC = () => (
  <div className="space-y-4">
    <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
      <h1 className="text-xl font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
        <Receipt className="text-blue-600 dark:text-blue-400" size={24} />
        Sales History &amp; Audit Log
      </h1>
      <p className="text-xs text-slate-500 mt-1">
        Browse customer invoices, payments and balances. Bills are never edited or deleted.
      </p>
    </div>
    <BillHistoryView kind="SALE" />
  </div>
);

export default SalesHistoryPage;
