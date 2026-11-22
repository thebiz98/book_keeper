# Dependency vulnerabilities

Checked 2026-11-22 with `npm audit` (read-only; `audit fix` was not run).

## Summary
`npm audit` reports 6 findings (1 high, 2 critical, 3 moderate). All are in the **vitest 2 dev/test dependency tree**.

- None of these packages ship in the product. The frontend bundle (`vite build`) and the Rust binary do not include them.
- They only matter on a developer machine while the Vitest or Vite dev server is running (for example, a malicious web page reaching the local dev server, or untrusted mock/config input).
- Not ARM-specific; the same findings appear on every platform.

## Findings

| Package | Severity | Path / why | Fixed in |
|---|---|---|---|
| `vitest` | critical | Direct devDependency (`^2.1.8`); range `<=4.1.10`. Pulls in the vulnerable packages below. | 4.1.12 |
| `tinypool` | critical | Via `vitest`. Prototype-pollution gadget in worker options leading to RCE (GHSA-5gmw-xhrv-c9v3, GHSA-85c8-ppgw-ccpr); range `<=2.1.1`. | with vitest 4.1.12 |
| `vite` | high | Via `vitest` and `vite-node` (nested copies); range `<=6.4.3`. Depends on vulnerable `esbuild`. | with vitest 4.1.12 |
| `@vitest/mocker` | moderate | Via `vitest`. Path traversal / arbitrary file read through redirect mock (GHSA-82fw-gwwq-j7x9); code injection via manual mock registration (GHSA-qxg6-89mg-rqv8); range `<=4.1.11`. | with vitest 4.1.12 |
| `vite-node` | moderate | Via `vitest`; range `<=2.2.0-beta.2`. Depends on vulnerable `vite`. | with vitest 4.1.12 |
| `esbuild` | moderate | Nested under `vitest` and `vite-node`; range `<=0.24.2`. Any website can send requests to the dev server and read the response (GHSA-67mh-4wv8-2f99). | with vitest 4.1.12 |

The only fix npm offers is `npm audit fix --force`, which installs vitest 4.1.12 (a breaking major upgrade).

## Fix plan (separate task, not done yet)
1. Upgrade `vitest` to 4.1.12. Expect changes to the config import and to the mocking API; rerun the whole suite (`npx vitest run`) and fix what breaks.
2. Add `npm audit --omit=dev` to the pre-release gate, so production dependencies are checked on every release. Dev-tree findings stay informational.

## Deferred non-security major upgrades
Not needed for any vulnerability; do them separately and only when there is a reason.
- Tailwind CSS 4 (currently 3.4)
- TypeScript 7 (currently 5.9)
- `lucide-react` 1.x (currently 0.468)
- `tailwind-merge` 3 (currently 2.5)
