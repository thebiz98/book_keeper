import React from 'react';
import { Truck } from 'lucide-react';
import { BillHistoryView } from '@/components/domain/BillHistoryView';

export const PurchaseHistoryPage: React.FC = () => (
  <div className="space-y-4">
    <div className="bg-white dark:bg-slate-900 p-4 rounded-lg border border-slate-200 dark:border-slate-800 shadow-xs">
      <h1 className="text-xl font-bold text-slate-900 dark:text-slate-100 flex items-center gap-2">
        <Truck className="text-blue-600 dark:text-blue-400" size={24} />
        Purchase History &amp; Lots
      </h1>
      <p className="text-xs text-slate-500 mt-1">
        Supplier shipments, remaining FIFO quantities and vendor payments.
      </p>
    </div>
    <BillHistoryView kind="PURCHASE" />
  </div>
);

export default PurchaseHistoryPage;
