import { beforeEach, describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

import { invoke } from '@tauri-apps/api/core';
import { invokeCmd } from '@/api/ipc';
import { errorMessage, isAppError } from '@/api/errors';

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
