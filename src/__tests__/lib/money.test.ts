import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { round2 } from '../../lib/money';

describe('round2 (L4, DD-97)', () => {
  it('matches the shared vectors the Rust round2 is tested against', () => {
    const file = resolve(__dirname, '../../../src-tauri/tests/fixtures/round2_vectors.json');
    const vectors = JSON.parse(readFileSync(file, 'utf-8')) as [number, number][];
    expect(vectors.length).toBeGreaterThan(10);
    for (const [input, expected] of vectors) {
      expect(Math.abs(round2(input) - expected), `round2(${input})`).toBeLessThan(1e-9);
    }
  });

  it('rounds half a paisa away from zero, both signs', () => {
    expect(round2(0.005)).toBe(0.01);
    expect(round2(-0.005)).toBe(-0.01);
    expect(round2(0.125)).toBe(0.13);
    expect(round2(0.004)).toBe(0);
  });
});
