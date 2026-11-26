import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { invokeCmd } from '@/api/ipc';
import { deletedReference, errorMessage, isAppError } from '@/api/errors';

const mocked = vi.mocked(invoke);

describe('invokeCmd error normalisation', () => {
  beforeEach(() => {
    mocked.mockReset();
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  it('passes through a typed object', async () => {
    const e = { type: 'NotFound', details: { entity: 'Item', id: '9' } };
    mocked.mockRejectedValue(e);
    await expect(invokeCmd('x')).rejects.toMatchObject(e);
  });

  it('parses a JSON string', async () => {
    mocked.mockRejectedValue(JSON.stringify({ type: 'DatabaseError', details: 'boom' }));
    const err = await invokeCmd<any>('x').catch((x): any => x);
    expect(isAppError(err)).toBe(true);
    expect(err.details).toBe('boom');
    expect(err.message).toBe('boom');
  });

  it('keeps snake_case details unconverted', async () => {
    mocked.mockRejectedValue({
      type: 'InsufficientStockError',
      details: { item_id: 1, item_name: 'Rice', required: 5, available: 2 },
    });
    const err = await invokeCmd<any>('x').catch((x): any => x);
    expect(err.details.item_name).toBe('Rice');
  });

  it('wraps plain strings and Errors', async () => {
    mocked.mockRejectedValue('plain failure');
    expect(await invokeCmd<any>('x').catch((x: any) => x)).toEqual({ type: 'IoError', details: 'plain failure' });
    mocked.mockRejectedValue(new Error('net'));
    expect(await invokeCmd<any>('x').catch((x: any) => x)).toEqual({ type: 'IoError', details: 'net' });
  });
});

describe('errorMessage', () => {
  it('prefers details.message', () => {
    expect(errorMessage({ type: 'ValidationError', details: { field: 'a', message: 'bad a' } })).toBe('bad a');
  });
  it('uses string details', () => {
    expect(errorMessage({ type: 'IoError', details: 'disk' })).toBe('disk');
  });
  it('builds fallbacks for struct variants', () => {
    expect(errorMessage({ type: 'NotFound', details: { entity: 'Item', id: '9' } })).toBe(
      'Entity not found: Item with identifier 9'
    );
    expect(
      errorMessage({
        type: 'InsufficientStockError',
        details: { item_id: 1, item_name: 'Rice', required: 5, available: 2 },
      })
    ).toBe("Insufficient stock for item 'Rice'. Required: 5, Available: 2");
  });
  it('falls back to Error.message and String()', () => {
    expect(errorMessage(new Error('e'))).toBe('e');
    expect(errorMessage(42)).toBe('42');
  });
});

describe('BillLocked (FR-20)', () => {
  it('is a typed error whose message is the guard reason', () => {
    const e = { type: 'BillLocked', details: { code: 'LINKED_RETURNS', message: 'This bill has returns (RET-1001).' } };
    expect(isAppError(e)).toBe(true);
    expect(errorMessage(e)).toBe('This bill has returns (RET-1001).');
  });

  it('keeps code and message unconverted through invokeCmd', async () => {
    mocked.mockReset();
    vi.spyOn(console, 'error').mockImplementation(() => {});
    mocked.mockRejectedValue({ type: 'BillLocked', details: { code: 'NEWER_RETURN', message: 'Delete RET-1002 first.' } });
    const err = await invokeCmd<any>('delete_return').catch((x): any => x);
    expect(err.details.code).toBe('NEWER_RETURN');
    expect(err.message).toBe('Delete RET-1002 first.');
  });

  it('sends camelCase args and the enum target as is', async () => {
    mocked.mockReset();
    mocked.mockResolvedValue({ target: 'SALE', id: 1, can_edit: true, lot_limits: [] });
    const g = await invokeCmd<any>('get_bill_edit_guard', { target: 'SALE', id: 1 });
    expect(mocked).toHaveBeenCalledWith('get_bill_edit_guard', { target: 'SALE', id: 1 });
    expect(g.canEdit).toBe(true);
    expect(g.lotLimits).toEqual([]);
  });

  it('DeletedReference (DD-90) is typed and carries what Restore needs', async () => {
    mocked.mockReset();
    vi.spyOn(console, 'error').mockImplementation(() => {});
    const msg = "'Ravi' is deleted. Restore it first.";
    mocked.mockRejectedValue({ type: 'DeletedReference', details: { entity: 'PARTY', id: 4, name: 'Ravi', message: msg } });
    const err = await invokeCmd<any>('update_sale').catch((x): any => x);
    expect(isAppError(err)).toBe(true);
    expect(errorMessage(err)).toBe(msg);
    expect(deletedReference(err)).toEqual({ entity: 'PARTY', id: 4, name: 'Ravi', message: msg });
    expect(deletedReference(new Error('x'))).toBeNull();
  });
});
