import React, { useState, useEffect } from 'react';
import { User, Truck, Search } from 'lucide-react';
import { api } from '@/api';
import { useAppStore } from '@/store/useAppStore';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { SettlementModal } from '@/components/domain/SettlementModal';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';

export const CreditDuesPage: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'CUSTOMER' | 'VENDOR'>('CUSTOMER');
  const [duesList, setDuesList] = useState<any[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);

  // Settlement modal state
  const [selectedParty, setSelectedParty] = useState<any>(null);
  const [isSettlementOpen, setIsSettlementOpen] = useState(false);

  const recordCashFlow = useAppStore((s) => s.recordCashFlow);

  const fetchDues = () => {
    setIsLoading(true);
    api
      .getPartyDuesSummary(activeTab)
      .then((data) => {
        if (Array.isArray(data)) {
          setDuesList(data);
        } else {
          setDuesList([]);
        }
      })
      .catch(() => setDuesList([]))
      .finally(() => setIsLoading(false));
  };

  useEffect(() => {
    fetchDues();
  }, [activeTab]);

  const filteredDues = duesList.filter((d) => {
    if (!searchQuery.trim()) return true;
    const q = searchQuery.toLowerCase();
    const name = (d.partyName || d.party_name || d.name || '').toLowerCase();
    const phone = (d.phone || '').toLowerCase();
    return name.includes(q) || phone.includes(q);
  });

  const totalOutstanding = filteredDues.reduce((acc, d) => {
    const val = d.netDueBalance ?? d.balance_due ?? d.totalDue ?? 0;
    return acc + val;
  }, 0);

  const handleSettled = (settledAmount: number) => {
    if (activeTab === 'CUSTOMER') {
      recordCashFlow({ customerDuesSettled: settledAmount });
    } else {
      recordCashFlow({ vendorDuesSettled: settledAmount });
    }
    fetchDues();
  };

  return (
    <div className="flex flex-col gap-4">
      {/* Top Header & Tab Ribbon */}
      <div className="p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <div className="flex p-0.5 rounded-lg border border-slate-200 dark:border-slate-700 bg-slate-100 dark:bg-slate-800 text-xs">
            <button
              type="button"
              onClick={() => setActiveTab('CUSTOMER')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md font-semibold transition-colors cursor-pointer ${
                activeTab === 'CUSTOMER'
                  ? 'bg-white dark:bg-slate-900 text-rose-600 dark:text-rose-400 shadow-2xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
              }`}
            >
              <User className="h-3.5 w-3.5" />
              <span>Customer Receivables</span>
            </button>

            <button
              type="button"
              onClick={() => setActiveTab('VENDOR')}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md font-semibold transition-colors cursor-pointer ${
                activeTab === 'VENDOR'
                  ? 'bg-white dark:bg-slate-900 text-amber-600 dark:text-amber-400 shadow-2xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900'
              }`}
            >
              <Truck className="h-3.5 w-3.5" />
              <span>Supplier Payables</span>
            </button>
          </div>
        </div>

        {/* Total Dues Callout */}
        <div className="flex items-center gap-4">
          <div className="flex flex-col text-right">
            <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
              {activeTab === 'CUSTOMER' ? 'Total Receivables Owed' : 'Total Payables Due'}
            </span>
            <div className="text-xl font-bold font-mono text-rose-600 dark:text-rose-400">
              <CurrencyDisplay amount={totalOutstanding} size="xl" colored />
            </div>
          </div>
        </div>
      </div>

      {/* Filter Bar */}
      <div className="p-3 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex items-center justify-between gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
          <input
            type="text"
            placeholder="Search party by name or mobile phone..."
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full text-xs pl-8 pr-3 py-1.5 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>

        <span className="text-xs text-slate-500 font-mono">
          {filteredDues.length} Unsettled Accounts
        </span>
      </div>

      {/* Dues Ledger Table */}
      <div className="rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs flex-1">
        <Table dense>
          <TableHeader>
            <TableRow>
              <TableHead className="w-12 text-center">#</TableHead>
              <TableHead>Party Name</TableHead>
              <TableHead>Phone / Contact</TableHead>
              <TableHead className="text-center">Unsettled Bills</TableHead>
              <TableHead align="right">Outstanding Balance (₹)</TableHead>
              <TableHead className="w-28 text-center">Action</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={6} className="py-12 text-center text-slate-400 text-xs">
                  Loading khata ledger...
                </TableCell>
              </TableRow>
            ) : filteredDues.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="py-12 text-center text-slate-400 text-xs">
                  All accounts settled! Zero outstanding dues.
                </TableCell>
              </TableRow>
            ) : (
              filteredDues.map((due, idx) => {
                const name = due.partyName || due.party_name || due.name || 'Account';
                const phone = due.phone || '—';
                const count = due.unsettledBillsCount ?? due.billCount ?? 1;
                const balance = due.netDueBalance ?? due.balance_due ?? due.totalDue ?? 0;

                return (
                  <TableRow key={due.partyId || due.id || idx}>
                    <TableCell className="text-center text-slate-400 select-none">
                      {idx + 1}
                    </TableCell>
                    <TableCell className="font-semibold text-slate-900 dark:text-slate-100">
                      {name}
                    </TableCell>
                    <TableCell mono className="text-xs text-slate-500">
                      {phone}
                    </TableCell>
                    <TableCell className="text-center font-mono text-xs">
                      <span className="px-2 py-0.5 rounded bg-slate-100 dark:bg-slate-800 font-semibold">
                        {count} Bills
                      </span>
                    </TableCell>
                    <TableCell align="right" mono className="font-bold text-rose-600 dark:text-rose-400 text-sm">
                      <CurrencyDisplay amount={balance} size="md" colored />
                    </TableCell>
                    <TableCell className="text-center">
                      <Button
                        variant="primary"
                        size="xs"
                        onClick={() => {
                          setSelectedParty({
                            id: due.partyId || due.id,
                            name,
                            totalDue: balance,
                          });
                          setIsSettlementOpen(true);
                        }}
                      >
                        Settle
                      </Button>
                    </TableCell>
                  </TableRow>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>

      {/* FIFO Settlement Modal */}
      {selectedParty && (
        <SettlementModal
          isOpen={isSettlementOpen}
          onClose={() => setIsSettlementOpen(false)}
          partyId={selectedParty.id}
          partyName={selectedParty.name}
          totalDue={selectedParty.totalDue}
          onSettled={handleSettled}
        />
      )}
    </div>
  );
};

export { CreditDuesPage as DuesPage };
export default CreditDuesPage;
