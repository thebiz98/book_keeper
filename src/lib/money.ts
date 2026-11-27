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

const ONES = [
  '', 'One', 'Two', 'Three', 'Four', 'Five', 'Six', 'Seven', 'Eight', 'Nine', 'Ten',
  'Eleven', 'Twelve', 'Thirteen', 'Fourteen', 'Fifteen', 'Sixteen', 'Seventeen', 'Eighteen', 'Nineteen',
];
const TENS = ['', '', 'Twenty', 'Thirty', 'Forty', 'Fifty', 'Sixty', 'Seventy', 'Eighty', 'Ninety'];

/** 0..99 in words ("" for 0). */
function twoDigits(n: number): string {
  if (n < 20) return ONES[n];
  return [TENS[Math.floor(n / 10)], ONES[n % 10]].filter(Boolean).join(' ');
}

/** 0..999 in words ("" for 0). */
function threeDigits(n: number): string {
  const h = Math.floor(n / 100);
  return [h ? `${ONES[h]} Hundred` : '', twoDigits(n % 100)].filter(Boolean).join(' ');
}

/** A whole number in words with Indian grouping (crore, lakh, thousand). */
function wholeInWords(n: number): string {
  if (n === 0) return 'Zero';
  const crore = Math.floor(n / 10_000_000);
  const rest = n % 10_000_000;
  const parts = [
    crore ? `${wholeInWords(crore)} Crore` : '',
    Math.floor(rest / 100_000) ? `${twoDigits(Math.floor(rest / 100_000))} Lakh` : '',
    Math.floor((rest % 100_000) / 1000) ? `${twoDigits(Math.floor((rest % 100_000) / 1000))} Thousand` : '',
    threeDigits(rest % 1000),
  ];
  return parts.filter(Boolean).join(' ');
}

/** Bill total in words, e.g. 530.5 -> "Rupees Five Hundred Thirty and Fifty Paise Only" (display only). */
export function amountInWords(n: number): string {
  const paiseTotal = Math.round(Math.abs(round2(n)) * 100);
  const rupees = Math.floor(paiseTotal / 100);
  const paise = paiseTotal % 100;
  const sign = n < 0 && paiseTotal > 0 ? 'Minus ' : '';
  const paisePart = paise ? ` and ${twoDigits(paise)} Paise` : '';
  return `${sign}Rupees ${wholeInWords(rupees)}${paisePart} Only`;
}
