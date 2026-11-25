import React, { useState, useEffect, useRef } from 'react';
import { useLiveSearch, LIST_SEARCH_DEBOUNCE_MS } from '@/hooks/useLiveSearch';
import { Users, Plus, Search, Pencil, User, Truck, BookOpen, Upload } from 'lucide-react';
import { api, errorMessage } from '@/api';
import type { PartyListRow, PartyType } from '@/types';
import type { CreatePartyPayload } from '@/types/api';
import { todayLocal } from '@/lib/dates';
import { PartyLedgerDrawer } from '@/components/domain/PartyLedgerDrawer';
import { ImportDialog } from '@/components/domain/ImportDialog';
import { useAppStore } from '@/store/useAppStore';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { TextInput } from '@/components/ui/TextInput';
import { Select } from '@/components/ui/Select';
import { Tabs } from '@/components/ui/Tabs';
import { FormActions, enterToSubmit } from '@/components/ui/FormActions';

interface PartyForm {
  id: number | null;
  name: string;
  phone: string;
  notes: string;
  partyType: PartyType;
  /** Opening balance (FR-35): amount text, "As of" date, and what the existing opening bill shows. */
  openingAmount: string;
  openingDate: string;
  hadOpening: boolean;
  settledAmount: number;
}

const emptyForm = (partyType: PartyType): PartyForm => ({
  id: null,
  name: '',
  phone: '',
  notes: '',
  partyType,
  openingAmount: '',
  openingDate: todayLocal(),
  hadOpening: false,
  settledAmount: 0,
});

export const PartiesPage: React.FC = () => {
  const [tab, setTab] = useState<PartyType>('CUSTOMER');
  const [ledgerId, setLedgerId] = useState<number | null>(null);
  const [importOpen, setImportOpen] = useState(false);
  const refreshMetrics = useAppStore((s) => s.refreshMetrics);

  const [form, setForm] = useState<PartyForm | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const savingRef = useRef(false);
  const formOpen = form !== null;

  useEffect(() => {
    setFormError(null);
  }, [formOpen, form?.id]);

  const live = useLiveSearch<PartyListRow[]>({
    initial: [],
    emptyBehavior: 'fetch',
    debounceMs: LIST_SEARCH_DEBOUNCE_MS,
    deps: [tab],
    fetcher: (q) => (q ? api.searchParties(q, tab) : api.listParties(tab, true)),
  });
  const { query, setQuery, isLoading: fetching, refresh: load } = live;
  const parties = live.error ? [] : live.results;
  const isLoading = fetching && live.resultsFor === null;
  const error = live.error ? `Failed to load parties: ${errorMessage(live.error)}` : null;

  const handleSave = async () => {
    if (!form || savingRef.current) return;
    if (!form.name.trim()) {
      setFormError('Party name is required');
      return;
    }
    savingRef.current = true;
    setIsSaving(true);
    setFormError(null);
    const payload: CreatePartyPayload = {
      name: form.name.trim(),
      phone: form.phone.trim() || null,
      notes: form.notes.trim() || null,
      partyType: form.partyType,
    };
    const amountText = form.openingAmount.trim();
    if (amountText) {
      const amount = Number(amountText);
      if (!Number.isFinite(amount)) {
        setFormError('Opening balance must be a number');
        savingRef.current = false;
        setIsSaving(false);
        return;
      }
      payload.openingBalance = { amount, asOfDate: form.openingDate };
    } else if (form.hadOpening) {
      // An emptied field of an existing opening bill removes it (the backend refuses if settled).
      payload.openingBalance = { amount: 0, asOfDate: form.openingDate };
    }
    try {
      if (form.id === null) {
        await api.createParty(payload);
      } else {
        await api.updateParty(form.id, payload);
      }
      setForm(null);
      load();
    } catch (err) {
      setFormError(errorMessage(err));
    } finally {
      savingRef.current = false;
      setIsSaving(false);
    }
  };

  const editParty = (p: PartyListRow) =>
    setForm({
      id: p.id,
      name: p.name,
      phone: p.phone ?? '',
      notes: p.notes ?? '',
      partyType: p.partyType,
      openingAmount: p.openingBalance ? String(p.openingBalance.amount) : '',
      openingDate: p.openingBalance?.asOfDate ?? todayLocal(),
      hadOpening: !!p.openingBalance,
      settledAmount: p.openingBalance?.settledAmount ?? 0,
    });

  return (
    <div className="flex flex-col gap-4 h-full min-h-0">
      <div className="shrink-0 p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Users className="h-5 w-5 text-blue-600" />
          <h2 className="text-sm font-bold">Parties Directory</h2>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="md"
            leftIcon={<Upload className="h-4 w-4" />}
            onClick={() => setImportOpen(true)}
          >
            Import
          </Button>
          <Button
            variant="primary"
            size="md"
            leftIcon={<Plus className="h-4 w-4" />}
            onClick={() => setForm(emptyForm(tab))}
          >
            Add Party
          </Button>
        </div>
      </div>
      <ImportDialog
        kind="PARTIES"
        isOpen={importOpen}
        onClose={() => setImportOpen(false)}
        onImported={() => {
          load();
          void refreshMetrics();
        }}
      />

      <div className="shrink-0 flex flex-wrap items-end justify-between gap-3">
        <Tabs
          tabs={[
            { id: 'CUSTOMER', label: 'Customers', icon: <User className="h-3.5 w-3.5" /> },
            { id: 'VENDOR', label: 'Vendors', icon: <Truck className="h-3.5 w-3.5" /> },
          ]}
          activeTab={tab}
          onChange={(id) => setTab(id === 'VENDOR' ? 'VENDOR' : 'CUSTOMER')}
        />
        <div className="w-64">
          <TextInput
            placeholder="Search by name or phone..."
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            prefixIcon={<Search className="h-4 w-4" />}
          />
        </div>
      </div>

      {error && (
        <p role="alert" className="shrink-0 text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}

      <div className="rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs flex-1 min-h-[240px] flex flex-col">
        <Table dense scroll="fill">
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Phone</TableHead>
              <TableHead>Address / Notes</TableHead>
              <TableHead align="right">Transactions</TableHead>
              <TableHead align="right">Outstanding Balance</TableHead>
              <TableHead className="w-32 text-center">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {parties.length === 0 ? (
              <TableRow>
                <TableCell colSpan={6} className="text-center text-slate-400 py-8">
                  {isLoading ? 'Loading...' : 'No parties found'}
                </TableCell>
              </TableRow>
            ) : (
              parties.map((p) => (
                <TableRow key={p.id}>
                  <TableCell className="font-medium">{p.name}</TableCell>
                  <TableCell className="font-mono">{p.phone || '-'}</TableCell>
                  <TableCell>{p.notes || '-'}</TableCell>
                  <TableCell align="right" mono>
                    {p.transactionCount}
                  </TableCell>
                  <TableCell align="right">
                    <CurrencyDisplay amount={p.outstandingBalance} colored size="sm" />
                  </TableCell>
                  <TableCell className="text-center">
                    <div className="flex justify-center gap-1">
                      <Button variant="ghost" size="xs" title="Edit" onClick={() => editParty(p)}>
                        <Pencil className="h-3.5 w-3.5" />
                      </Button>
                      <Button variant="ghost" size="xs" title="View Transaction History" onClick={() => setLedgerId(p.id)}>
                        <BookOpen className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      <PartyLedgerDrawer partyId={ledgerId} onClose={() => setLedgerId(null)} />

      <Modal
        isOpen={form !== null}
        onClose={() => setForm(null)}
        title={form?.id == null ? 'Add Party' : 'Edit Party'}
        size="md"
        footer={
          <FormActions
            onSave={() => void handleSave()}
            onCancel={() => setForm(null)}
            isSubmitting={isSaving}
            canSave={!!form?.name.trim()}
          />
        }
      >
        {form && (
          <div className="flex flex-col gap-3" onKeyDown={enterToSubmit(() => void handleSave())}>
            <TextInput
              label="Name"
              value={form.name}
              autoFocus
              onChange={(e) => setForm({ ...form, name: e.target.value })}
            />
            <TextInput
              label="Phone"
              value={form.phone}
              onChange={(e) => setForm({ ...form, phone: e.target.value })}
            />
            <TextInput
              label="Notes"
              value={form.notes}
              onChange={(e) => setForm({ ...form, notes: e.target.value })}
            />
            <Select
              label="Type"
              value={form.partyType}
              onChange={(v) => setForm({ ...form, partyType: v })}
              disabled={form.hadOpening}
              options={[
                { value: 'CUSTOMER', label: 'Customer' },
                { value: 'VENDOR', label: 'Vendor' },
              ]}
            />
            <fieldset className="flex flex-col gap-2 rounded-md border border-slate-200 dark:border-slate-700 p-3">
              <legend className="px-1 text-[11px] font-semibold uppercase tracking-wide text-slate-500">
                Opening balance
              </legend>
              <div className="grid grid-cols-2 gap-3">
                <TextInput
                  label={form.partyType === 'CUSTOMER' ? 'Customer owes you (₹)' : 'You owe this vendor (₹)'}
                  type="number"
                  min={0}
                  step="0.01"
                  inputMode="decimal"
                  value={form.openingAmount}
                  onChange={(e) => setForm({ ...form, openingAmount: e.target.value })}
                />
                <TextInput
                  label="As of"
                  type="date"
                  max={todayLocal()}
                  value={form.openingDate}
                  onChange={(e) => setForm({ ...form, openingDate: e.target.value })}
                />
              </div>
              {form.settledAmount > 0 && (
                <p data-testid="opening-settled" className="text-xs text-slate-500">
                  ₹{form.settledAmount} already settled
                </p>
              )}
            </fieldset>
            {formError && (
              <p role="alert" className="text-xs text-rose-600 dark:text-rose-400">
                {formError}
              </p>
            )}
          </div>
        )}
      </Modal>
    </div>
  );
};

export default PartiesPage;
