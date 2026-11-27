import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { amountInWords, round2 } from '../../lib/money';

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

describe('amountInWords (printed bill)', () => {
  it('spells rupees with Indian grouping and paise', () => {
    expect(amountInWords(0)).toBe('Rupees Zero Only');
    expect(amountInWords(530)).toBe('Rupees Five Hundred Thirty Only');
    expect(amountInWords(12345678.5)).toBe(
      'Rupees One Crore Twenty Three Lakh Forty Five Thousand Six Hundred Seventy Eight and Fifty Paise Only'
    );
    expect(amountInWords(10000000)).toBe('Rupees One Crore Only');
    expect(amountInWords(100000)).toBe('Rupees One Lakh Only');
    expect(amountInWords(19.99)).toBe('Rupees Nineteen and Ninety Nine Paise Only');
    expect(amountInWords(-15)).toBe('Minus Rupees Fifteen Only');
  });
});
