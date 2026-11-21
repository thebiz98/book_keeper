import React, { useState, useEffect, useCallback } from 'react';
import { Users, Plus, Search, Pencil, User, Truck } from 'lucide-react';
import { api } from '@/api';
import type { Party, PartyType } from '@/types';
import { CurrencyDisplay } from '@/components/common/CurrencyDisplay';
import { Table, TableHeader, TableBody, TableRow, TableHead, TableCell } from '@/components/ui/Table';
import { Button } from '@/components/ui/Button';
import { Modal } from '@/components/ui/Modal';
import { TextInput } from '@/components/ui/TextInput';
import { Select } from '@/components/ui/Select';
import { Tabs } from '@/components/ui/Tabs';

interface PartyForm {
  id: number | null;
  name: string;
  phone: string;
  notes: string;
  partyType: PartyType;
}

const emptyForm = (partyType: PartyType): PartyForm => ({
  id: null,
  name: '',
  phone: '',
  notes: '',
  partyType,
});

export const PartiesPage: React.FC = () => {
  const [tab, setTab] = useState<PartyType>('CUSTOMER');
  const [query, setQuery] = useState('');
  const [parties, setParties] = useState<Party[]>([]);
  const [dues, setDues] = useState<Record<number, number>>({});
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [form, setForm] = useState<PartyForm | null>(null);
  const [isSaving, setIsSaving] = useState(false);

  const load = useCallback(() => {
    setIsLoading(true);
    const trimmed = query.trim();
    const req = trimmed ? api.searchParties(trimmed, tab) : api.listParties(tab, true);
    api
      .getPartyDuesSummary(tab)
      .then((rows) => setDues(Object.fromEntries(rows.map((r) => [r.partyId, r.balanceDue]))))
      .catch(() => setDues({}));
    req
      .then((data) => setParties(data))
      .catch((err: any) => {
        setParties([]);
        setError(err?.message || 'Failed to load parties');
      })
      .finally(() => setIsLoading(false));
  }, [tab, query]);

  useEffect(() => {
    const timer = setTimeout(load, 200);
    return () => clearTimeout(timer);
  }, [load]);

  const handleSave = async () => {
    if (!form) return;
    if (!form.name.trim()) {
      setError('Party name is required');
      return;
    }
    setIsSaving(true);
    setError(null);
    const payload = {
      name: form.name.trim(),
      phone: form.phone.trim() || null,
      notes: form.notes.trim() || null,
      partyType: form.partyType,
    };
    try {
      if (form.id === null) {
        await api.createParty(payload);
      } else {
        await api.updateParty(form.id, payload);
      }
      setForm(null);
      load();
    } catch (err: any) {
      setError(err?.message || 'Failed to save party');
    } finally {
      setIsSaving(false);
    }
  };

  const editParty = (p: Party) =>
    setForm({
      id: p.id,
      name: p.name,
      phone: p.phone ?? '',
      notes: p.notes ?? '',
      partyType: p.partyType,
    });

  return (
    <div className="flex flex-col gap-4">
      <div className="p-3.5 rounded-lg bg-white dark:bg-slate-900 border border-slate-200 dark:border-slate-800 shadow-xs flex flex-wrap items-center justify-between gap-3">
        <div className="flex items-center gap-2">
          <Users className="h-5 w-5 text-blue-600" />
          <h2 className="text-sm font-bold">Parties Directory</h2>
        </div>
        <Button
          variant="primary"
          size="md"
          leftIcon={<Plus className="h-4 w-4" />}
          onClick={() => setForm(emptyForm(tab))}
        >
          Add Party
        </Button>
      </div>

      <div className="flex flex-wrap items-end justify-between gap-3">
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
        <p role="alert" className="text-xs text-rose-600 dark:text-rose-400 font-medium px-1">
          {error}
        </p>
      )}

      <div className="rounded-lg border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 overflow-hidden shadow-xs">
        <Table dense>
          <TableHeader>
            <TableRow>
              <TableHead>Name</TableHead>
              <TableHead>Phone</TableHead>
              <TableHead>Notes</TableHead>
              <TableHead align="right">Balance Due</TableHead>
              <TableHead className="w-24 text-center">Actions</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {parties.length === 0 ? (
              <TableRow>
                <TableCell colSpan={5} className="text-center text-slate-400 py-8">
                  {isLoading ? 'Loading...' : 'No parties found'}
                </TableCell>
              </TableRow>
            ) : (
              parties.map((p) => (
                <TableRow key={p.id}>
                  <TableCell className="font-medium">{p.name}</TableCell>
                  <TableCell className="font-mono">{p.phone || '-'}</TableCell>
                  <TableCell>{p.notes || '-'}</TableCell>
                  <TableCell align="right">
                    <CurrencyDisplay
                      amount={dues[p.id] ?? 0}
                      colored
                      size="sm"
                    />
                  </TableCell>
                  <TableCell className="text-center">
                    <div className="flex justify-center gap-1">
                      <Button variant="ghost" size="xs" title="Edit" onClick={() => editParty(p)}>
                        <Pencil className="h-3.5 w-3.5" />
                      </Button>
                    </div>
                  </TableCell>
                </TableRow>
              ))
            )}
          </TableBody>
        </Table>
      </div>

      <Modal
        isOpen={form !== null}
        onClose={() => setForm(null)}
        title={form?.id == null ? 'Add Party' : 'Edit Party'}
        size="md"
        footer={
          <>
            <Button variant="outline" onClick={() => setForm(null)}>
              Cancel
            </Button>
            <Button variant="primary" isLoading={isSaving} onClick={handleSave}>
              Save
            </Button>
          </>
        }
      >
        {form && (
          <div className="flex flex-col gap-3">
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
              options={[
                { value: 'CUSTOMER', label: 'Customer' },
                { value: 'VENDOR', label: 'Vendor' },
              ]}
            />
          </div>
        )}
      </Modal>
    </div>
  );
};

export default PartiesPage;
