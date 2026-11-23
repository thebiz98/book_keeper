import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { PartyLedgerView } from '@/types';
import { Drawer } from '../ui/Drawer';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { CurrencyDisplay } from '../common/CurrencyDisplay';
import { BillDetailDrawer } from './BillDetailDrawer';

export interface PartyLedgerDrawerProps {
  /** null = closed. */
  partyId: number | null;
  onClose: () => void;
}

export const PartyLedgerDrawer: React.FC<PartyLedgerDrawerProps> = ({ partyId, onClose }) => {
  const [ledger, setLedger] = useState<PartyLedgerView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [reloadKey, setReloadKey] = useState(0);
  const [bill, setBill] = useState<{ kind: 'SALE' | 'PURCHASE'; id: number } | null>(null);

  useEffect(() => {
    if (partyId === null) return;
    let live = true;
    setError(null);
    setLoading(true);
    api
      .getPartyLedger(partyId)
      .then((l) => live && setLedger(l))
      .catch((e) => live && setError(errorMessage(e)))
      .finally(() => live && setLoading(false));
    return () => {
      live = false;
    };
  }, [partyId, reloadKey]);

  useEffect(() => {
    setLedger(null);
  }, [partyId]);

  /** A ledger row that belongs to a bill opens that bill (id found through the bills list). */
  const openBill = (code: string, type: string) => {
    if (type !== 'SALE' && type !== 'PURCHASE') return;
    const b = ledger?.bills.find((x) => x.transactionCode === code);
    if (b) setBill({ kind: type, id: b.id });
  };

  return (
    <Drawer
      isOpen={partyId !== null}
      onClose={onClose}
      title={ledger ? `Ledger - ${ledger.party.name}` : 'Party ledger'}
      description="Bills, payments, credits and refunds in date order"
      width="xl"
    >
      {loading && <p className="text-xs text-slate-500">Loading ledger...</p>}
      {error && (
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
          {error}
        </p>
      )}
      {ledger && (
        <div className="flex flex-col gap-3 text-xs">
          <div className="flex items-center justify-between p-3 rounded-md bg-slate-50 dark:bg-slate-800">
            <span className="text-slate-500">Current balance</span>
            <CurrencyDisplay amount={ledger.currentBalance} size="lg" colored />
          </div>
          {ledger.entries.length === 0 ? (
            <p className="text-slate-500">No ledger entries yet.</p>
          ) : (
            <Table dense scroll="page">
              <TableHeader>
                <TableRow className="text-left text-slate-400">
                  <TableHead className="py-1">Date</TableHead>
                  <TableHead>Ref</TableHead>
                  <TableHead>Type</TableHead>
                  <TableHead className="text-right">Debit</TableHead>
                  <TableHead className="text-right">Credit</TableHead>
                  <TableHead className="text-right">Balance</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {ledger.entries.map((e, i) => (
                  <TableRow
                    key={i}
                    onClick={() => openBill(e.transactionCode, e.transactionType)}
                    className={e.transactionType === 'SALE' || e.transactionType === 'PURCHASE' ? 'cursor-pointer hover:bg-slate-50 dark:hover:bg-slate-800' : undefined}
                  >
                    <TableCell className="py-1 font-mono">{e.date}</TableCell>
                    <TableCell className="font-mono">{e.transactionCode}</TableCell>
                    <TableCell>{e.transactionType}</TableCell>
                    <TableCell className="text-right">{e.debit > 0 ? <CurrencyDisplay amount={e.debit} size="sm" /> : ''}</TableCell>
                    <TableCell className="text-right">{e.credit > 0 ? <CurrencyDisplay amount={e.credit} size="sm" /> : ''}</TableCell>
                    <TableCell className="text-right">
                      <CurrencyDisplay amount={e.runningBalance} size="sm" />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </div>
      )}
      <BillDetailDrawer
        kind={bill?.kind ?? 'SALE'}
        billId={bill?.id ?? null}
        onClose={() => setBill(null)}
        onChanged={() => setReloadKey((k) => k + 1)}
      />
    </Drawer>
  );
};

export default PartyLedgerDrawer;
