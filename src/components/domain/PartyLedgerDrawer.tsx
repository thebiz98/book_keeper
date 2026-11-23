import React, { useEffect, useState } from 'react';
import { api, errorMessage } from '@/api';
import type { PartyLedgerView } from '@/types';
import { Drawer } from '../ui/Drawer';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '../ui/Table';
import { CurrencyDisplay } from '../common/CurrencyDisplay';

export interface PartyLedgerDrawerProps {
  /** null = closed. */
  partyId: number | null;
  onClose: () => void;
}

export const PartyLedgerDrawer: React.FC<PartyLedgerDrawerProps> = ({ partyId, onClose }) => {
  const [ledger, setLedger] = useState<PartyLedgerView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (partyId === null) return;
    let live = true;
    setLedger(null);
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
  }, [partyId]);

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
                  <TableRow key={i} >
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
    </Drawer>
  );
};

export default PartyLedgerDrawer;
