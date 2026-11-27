/**
 * Round to 2 decimals, half away from zero. The same function as Rust `round2`
 * (`(x * 100).round() / 100`); shared vectors in `src-tauri/tests/fixtures/round2_vectors.json`.
 */
export function round2(n: number): number {
  return (Math.sign(n) * Math.round(Math.abs(n) * 100)) / 100;
}

const inr = new Intl.NumberFormat('en-IN', {
  style: 'currency',
  currency: 'INR',
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

export function formatINR(n: number): string {
  return inr.format(round2(n));
}
