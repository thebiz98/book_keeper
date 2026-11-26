import React, { useCallback, useEffect, useState } from 'react';
import { User, Truck, Search, ChevronDown, ChevronRight, History } from 'lucide-react';
import { api, errorMessage } from '@/api';
import type { PartyDueSummary, PartyType } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { PaymentStatusBadge } from '@/components/common/PaymentStatusBadge';
import { SettlementModal } from '@/components/domain/SettlementModal';
import { SettlementHistoryView } from '@/components/domain/SettlementHistoryView';
import { PartyLedgerDrawer } from '@/components/domain/PartyLedgerDrawer';
import { DataTable, type DataTableColumn } from '@/components/domain/DataTable';
import { fetchOpenBills, transactionTypeFor, type OpenBill } from '@/components/domain/openBills';
import { refreshAppMetrics } from '@/components/domain/navParams';
import { Button } from '@/components/ui/Button';
import { Tabs } from '@/components/ui/Tabs';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/Table';

interface SettleTarget {
  party: PartyDueSummary;
  /** null = lump sum on account. */
  bill: OpenBill | null;
}

const OpenBillsPanel: React.FC<{
  party: PartyDueSummary;
  reloadKey: number;
  onSettle: (bill: OpenBill) => void;
}> = ({ party, reloadKey, onSettle }) => {
  const [bills, setBills] = useState<OpenBill[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const type = transactionTypeFor(party.partyType);

  useEffect(() => {
    let live = true;
    setBills(null);
    setError(null);
    fetchOpenBills(type, party.partyId)
      .then((b) => live && setBills(b))
      .catch((e) => live && setError(errorMessage(e)));
    return () => {
      live = false;
    };
  }, [type, party.partyId, reloadKey]);

  if (error) {
    return (
      <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
        Could not load open bills: {error}
      </p>
    );
  }
  if (!bills) return <p className="text-xs text-slate-500">Loading open bills...</p>;
  if (bills.length === 0) return <p className="text-xs text-slate-500">No open bills.</p>;
  return (
    <Table dense scroll="none" data-testid="open-bills">
      <TableHeader>
        <TableRow className="text-left text-slate-400">
          <TableHead>Code</TableHead>
          <TableHead>Date</TableHead>
          <TableHead className="text-right">Total</TableHead>
          <TableHead className="text-right">Paid</TableHead>
          <TableHead className="text-right">Balance</TableHead>
          <TableHead className="text-center">Status</TableHead>
          <TableHead />
        </TableRow>
      </TableHeader>
      <TableBody>
        {bills.map((b) => (
          <TableRow key={b.id} >
            <TableCell className="py-1 font-mono font-semibold">
              {b.transactionCode}
              {b.isOpeningBalance && (
                <span data-testid="opening-label" className="ml-1.5 font-sans font-normal text-[11px] text-slate-500">
                  Opening balance
                </span>
              )}
            </TableCell>
            <TableCell className="font-mono">{b.transactionDate}</TableCell>
            <TableCell className="text-right">
              <CurrencyDisplay amount={b.totalAmount} size="sm" />
            </TableCell>
            <TableCell className="text-right">
              <CurrencyDisplay amount={b.paidAmount} size="sm" />
            </TableCell>
            <TableCell className="text-right">
              <CurrencyDisplay amount={b.balanceDue} size="sm" colored invertColors={party.partyType === 'VENDOR'} />
            </TableCell>
            <TableCell className="text-center">
              <PaymentStatusBadge status={b.paymentStatus} />
            </TableCell>
            <TableCell className="text-right">
              <Button variant="secondary" size="xs" onClick={() => onSettle(b)}>
                Settle
              </Button>
            </TableCell>
          </TableRow>
        ))}
      </TableBody>
    </Table>
  );
};

export const CreditDuesPage: React.FC = () => {
  const [activeTab, setActiveTab] = useState<PartyType>('CUSTOMER');
  /** FR-42: third tab. The party tab stays remembered underneath. */
  const [showHistory, setShowHistory] = useState(false);
  const [duesList, setDuesList] = useState<PartyDueSummary[]>([]);
  const [searchQuery, setSearchQuery] = useState('');
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [expanded, setExpanded] = useState<number | null>(null);
  const [reloadKey, setReloadKey] = useState(0);
  const [target, setTarget] = useState<SettleTarget | null>(null);
  const [ledgerPartyId, setLedgerPartyId] = useState<number | null>(null);

  const isCustomer = activeTab === 'CUSTOMER';
  const type = transactionTypeFor(activeTab);

  const fetchDues = useCallback(() => {
    let live = true;
    setIsLoading(true);
    setError(null);
    api
      .getPartyDuesSummary(activeTab)
      .then((data) => live && setDuesList(data))
      .catch((e) => {
        if (live) {
          setDuesList([]);
          setError(errorMessage(e));
        }
      })
      .finally(() => live && setIsLoading(false));
    return () => {
      live = false;
    };
  }, [activeTab]);

  useEffect(() => {
    setExpanded(null);
    return fetchDues();
  }, [fetchDues]);

  const q = searchQuery.trim().toLowerCase();
  const filtered = duesList.filter(
    (d) => !q || d.partyName.toLowerCase().includes(q) || (d.phone ?? '').toLowerCase().includes(q)
  );
  const totalOutstanding = filtered.reduce((acc, d) => acc + d.balanceDue, 0);

  const columns: DataTableColumn<PartyDueSummary>[] = [
    {
      key: 'name',
      header: isCustomer ? 'Customer' : 'Vendor',
      render: (d) => (
        <button
          type="button"
          className="flex items-center gap-1 font-semibold cursor-pointer"
          aria-label={`Toggle open bills for ${d.partyName}`}
          onClick={(e) => {
            e.stopPropagation();
            setExpanded(expanded === d.partyId ? null : d.partyId);
          }}
        >
          {expanded === d.partyId ? <ChevronDown className="h-3.5 w-3.5" /> : <ChevronRight className="h-3.5 w-3.5" />}
          {d.partyName}
          {d.isActive === false && (
            <span
              data-testid="party-deleted-badge"
              className="ml-1 rounded bg-slate-200 px-1.5 py-0.5 text-[10px] font-medium text-slate-600 dark:bg-slate-700 dark:text-slate-300"
            >
              Deleted
            </span>
          )}
        </button>
      ),
    },
    { key: 'phone', header: 'Phone', mono: true, render: (d) => d.phone ?? '-' },
    { key: 'count', header: 'Unsettled Bills', align: 'center', render: (d) => d.unsettledBillsCount },
    { key: 'earliest', header: 'Earliest Bill', mono: true, render: (d) => d.earliestBillDate },
    { key: 'inv', header: 'Total Invoiced', align: 'right', render: (d) => <CurrencyDisplay amount={d.totalInvoiced} /> },
    { key: 'paid', header: 'Total Paid', align: 'right', render: (d) => <CurrencyDisplay amount={d.totalPaid} /> },
    {
      key: 'bal',
      header: 'Net Due Balance',
      align: 'right',
      render: (d) => <CurrencyDisplay amount={d.balanceDue} colored invertColors={!isCustomer} className="font-bold" />,
    },
    {
      key: 'actions',
      header: 'Actions',
      align: 'right',
      render: (d) => (
        <div className="flex items-center justify-end gap-1.5" onClick={(e) => e.stopPropagation()}>
          <Button variant="primary" size="xs" onClick={() => setTarget({ party: d, bill: null })}>
            Settle bills
          </Button>
          <Button variant="ghost" size="xs" onClick={() => setLedgerPartyId(d.partyId)}>
            View Ledger
          </Button>
        </div>
      ),
    },
  ];

  return (
    <div className="flex flex-col gap-4 h-full min-h-0">
      <div className="shrink-0 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-3">
        <Tabs
          activeTab={showHistory ? 'HISTORY' : activeTab}
          onChange={(id) => {
            setShowHistory(id === 'HISTORY');
            if (id !== 'HISTORY') setActiveTab(id === 'VENDOR' ? 'VENDOR' : 'CUSTOMER');
          }}
          tabs={[
            { id: 'CUSTOMER', label: 'Customer Receivables', icon: <User className="h-3.5 w-3.5" /> },
            { id: 'VENDOR', label: 'Vendor Payables', icon: <Truck className="h-3.5 w-3.5" /> },
            { id: 'HISTORY', label: 'Settlement History', icon: <History className="h-3.5 w-3.5" /> },
          ]}
        />
        {!showHistory && (
        <div className="flex flex-col text-right">
          <span className="text-[11px] font-medium text-slate-500 uppercase tracking-wider">
            {isCustomer ? 'Total Receivables Owed' : 'Total Payables Due'}
          </span>
          {error ? <span className="text-xl text-slate-400">-</span> : <CurrencyDisplay amount={totalOutstanding} size="xl" colored invertColors={!isCustomer} />}
        </div>
        )}
      </div>

      {showHistory ? (
        <SettlementHistoryView
          onChanged={() => {
            setReloadKey((k) => k + 1);
            fetchDues();
          }}
        />
      ) : (
        <>
      <div className="shrink-0 p-3 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex items-center justify-between gap-3">
        <div className="relative flex-1 max-w-sm">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-4 w-4 text-slate-400" />
          <input
            type="text"
            aria-label="Search party"
            placeholder={`Search ${isCustomer ? 'customer' : 'vendor'} by name or phone...`}
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full text-xs pl-8 pr-3 py-1.5 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
        </div>
        <span className="text-xs text-slate-500 font-mono">{filtered.length} accounts with open bills</span>
      </div>

      <DataTable
        scroll="fill"
        className="flex-1 min-h-[240px]"
        columns={columns}
        items={filtered}
        rowKey={(d) => d.partyId}
        isLoading={isLoading}
        error={error}
        onRetry={fetchDues}
        emptyMessage="No outstanding dues. All accounts are settled."
        isExpanded={(d) => expanded === d.partyId}
        renderExpanded={(d) => (
          <OpenBillsPanel
            party={d}
            reloadKey={reloadKey}
            onSettle={(bill) => setTarget({ party: d, bill })}
          />
        )}
        onRowClick={(d) => setExpanded(expanded === d.partyId ? null : d.partyId)}
      />
        </>
      )}

      {target && (
        <SettlementModal
          isOpen
          onClose={() => setTarget(null)}
          partyId={target.party.partyId}
          partyName={target.party.partyName}
          totalDue={target.bill ? target.bill.balanceDue : target.party.balanceDue}
          transactionType={type}
          transactionId={target.bill ? target.bill.id : null}
          billCode={target.bill?.transactionCode}
          onSettled={() => {
            setTarget(null);
            setReloadKey((k) => k + 1);
            refreshAppMetrics();
            fetchDues();
          }}
        />
      )}

      <PartyLedgerDrawer partyId={ledgerPartyId} onClose={() => setLedgerPartyId(null)} />
    </div>
  );
};

export { CreditDuesPage as DuesPage };
export default CreditDuesPage;
