import { describe, it, expect } from 'vitest';
import { snakeToCamel, camelToSnake } from '@/api/caseConvert';

describe('caseConvert', () => {
  it('converts snake_case keys to camelCase deeply', () => {
    const out = snakeToCamel({
      total_count: 2,
      items: [{ base_unit: 'PCS', bundles: [{ units_per_bundle: 10 }] }],
      sale_item: null,
    });
    expect(out).toEqual({
      totalCount: 2,
      items: [{ baseUnit: 'PCS', bundles: [{ unitsPerBundle: 10 }] }],
      saleItem: null,
    });
  });

  it('converts camelCase keys to snake_case deeply', () => {
    const out = camelToSnake({
      partyId: 1,
      items: [{ itemId: 3, bundleId: null }],
    });
    expect(out).toEqual({ party_id: 1, items: [{ item_id: 3, bundle_id: null }] });
  });

  it('leaves primitives and arrays of primitives untouched', () => {
    expect(snakeToCamel('a_b')).toBe('a_b');
    expect(snakeToCamel(['x_y', 1])).toEqual(['x_y', 1]);
    expect(camelToSnake(null)).toBeNull();
  });
});
