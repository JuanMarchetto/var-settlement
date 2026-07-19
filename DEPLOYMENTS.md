# Deployments & on-chain evidence

> **On devnet, accounts are durable but transaction history is not.** Devnet prunes signatures after
> roughly two weeks, so any *tx link* here eventually 404s on Explorer while the *accounts* it
> created stay queryable forever. The durable, always-checkable evidence is therefore:
> the **program account**, the **settled market accounts**, and the **permissionless `reverify`
> command** — all listed below, all live right now. Signatures are listed as a same-day audit trail
> and were re-minted on **2026-07-19**.

## Devnet (live)

| Program | Address |
|---|---|
| **var_settlement** | [`AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk`](https://explorer.solana.com/address/AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk?cluster=devnet) |
| **mock_txoracle** (test only) | [`85KwDRzyZeG8wAXVCZo2CKTVor3qVcyhq7vk2yAzBJMw`](https://explorer.solana.com/address/85KwDRzyZeG8wAXVCZo2CKTVor3qVcyhq7vk2yAzBJMw?cluster=devnet) |
| **Txoracle** (real Tx LINE, devnet) | [`6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`](https://explorer.solana.com/address/6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J?cluster=devnet) |

- Binary: `target/deploy/var_settlement.so`, 355,512 bytes (347 KiB).
- Upgrade authority: `BExXP4aGnZRq3PMw4vpY7KHgdeGPCfogao66CpsTMUjf`

> `mock_txoracle` stands in for Tx LINE's `Txoracle::validate_stat` (always attests `true`) so the
> full lifecycle can run on devnet without consuming the live feed. **Never on mainnet, and not used
> in the real-feed settlement below.** A real market passes the real Tx LINE program id as
> `txoracle_program`.

## LIVE Tx LINE settlement — a real fixture settled against the on-chain Merkle feed — PASSED

The full real path, no mocks: `tests-devnet/txline-activate.ts` (4-step free World Cup activation) +
`tests-devnet/txline-settle.ts` (settle against the **real** `Txoracle`).

- **Settled market (durable evidence — open this):**
  [`GaiXEuSBb3spjoptxHCoyScycN4sCy164jCF3jT9v8T3`](https://explorer.solana.com/address/GaiXEuSBb3spjoptxHCoyScycN4sCy164jCF3jT9v8T3?cluster=devnet)
  — status `Settled`, `fixture_id` 18192996, receipt scoreline 2–3, outcome **Away**.
- **Anyone can re-derive it, from any wallet:**
  `cd tests-devnet && bun install && bun run reverify.ts GaiXEuSBb3spjoptxHCoyScycN4sCy164jCF3jT9v8T3`
  → `reverify (stranger wallet) -> true`. Verified from `WQMF7mLsD4CJ5UKCGHJFCMCdjce593xtjSnRi78gmv1`,
  a wallet that never created, funded, or resolved this market.
- **Fixture 18192996** (real): authenticated score **home 2 – 3 away** via live `stat-validation`
  Merkle proofs (base keys 1/2, period 4). Daily root PDA
  `CMtVGDyWsZ4u3yeYeyC9yxNzzyvwco6Jgtd9ubRJWCGV` (epoch day 20640) exists on devnet, 9232 bytes.
- **Two-step resolve** (both proofs won't fit one 1232-byte tx), 2026-07-19 run:

  | Step | Signature |
  |---|---|
  | `subscribe(1, 4)` — free World Cup activation | `4ZFya1o9yPCXMCdjDJXTEoHcsYChrhZQDNFYdnPrzPRRiwhENkFQ1yheRcw5PrAJx8GatZT1kSoqxakmdE2rqMDQ` |
  | `attest_home` — CPI authenticates home goals | `xuUExMAXeovpdcwMXegQMcpd49mampkqxWNdQze7Lf7ZzuKV1GbiyGppCSA1YbEXtD45byjF6ZS8zTAchba5MbJ` |
  | `resolve` — CPI authenticates away goals, rulebook resolves | `3uSfesCgBNN8qAjM2CwLjtCVPquaE3cUk2qkZQc7ndnKFbkStaqs26HNyZtPMFa22ZFj1ucuVSda1TNAeuwHxGYD` |

- **Result:** receipt outcome **Away** (2 < 3), `net` 98 USDC of a 100 USDC pot (2% fee), winning
  (Away) pool paid pro-rata — final balances 60 / 138. Needs a 1.4M compute-unit budget (Merkle
  verification is CU-heavy).

Re-run: `bun run txline-activate.ts && bun run txline-settle.ts 18192996 770`, then
`bun run reverify.ts <market-from-the-settle-output>`.

## End-to-end lifecycle smoke test (`tests-devnet/smoke.ts`) — PASSED

Full lifecycle on devnet with real transactions and real SPL-token transfers, against the **mock**
`Txoracle` (this test exercises the account/escrow/claim machinery; the real-feed authentication is
the section above): `create_market → deposit(Home 40, Away 60) → attest_home → resolve(Home 2-0) →
reverify → claim ×2`. 2026-07-19 run:

| Step | Signature |
|---|---|
| create_market | `61VQBCrUjX8hJThDAaozA4hsc3HYxv2Ds6Mj8ByTuHQynyRW49GX3j9ZAZgQXFk2HnXncoL9tCR85SuigvVMpmgg` |
| deposit A → Home 40 | `36axruEFj7kJtk65hyTXCU6zmHJSdriLgWYKA1zVq56qiEXq9TQR25DktMJi8cwz2dWFQDfKu9h2ULNQhyBndxaa` |
| deposit B → Away 60 | `47udgmKG2CzeeCNTAcKAQ3Qsr1BZJurapNH5cVcMu1sJv94BrrjoZyAapKbQMNMxrDF2V3imny4BW6kW1hhDtW1v` |
| attest_home | `67SSLyz2hLa168QAhwb3Cdpir2ewCCyFya1zCYx4Pr8WvgmF6bV3CKh4779k3abuVLhALuFRUD2YDmNmGXD1VxG1` |
| resolve (Home wins) | `21QqGESoPhHSrDjt4ZEEEM4CkiLK2UmZAEDJDaBTirEf3asMmcMF6wLqdBwkdjSgcqqcqMbmQSPSnqVmSmdutXod` |
| A claims (winner) | `5A8jAT6D5FC31Mu23R9JmD3px3FJ3DzsAhbkxf5munskCrBnuGvXoxqhYsCbKJaZKNiRWcPTXabkB3ygd2A7w63n` |
| B claims (loser) | `5k7asqoYbWXaaconaKDHxhxnmkG4oVd8Fprdo9fSrDm6xQeQLc8XQE7FjTSV924BJVt6BfpuEYNi9WEPrgVvzYb2` |

**Result:** receipt outcome `Home`, `reverify() -> true`, final balances userA=158, userB=40,
vault=2 (the 2% protocol fee) — exactly the settlement math the rulebook specifies. Exit 0.

Re-run: `cd tests-devnet && bun install && bun run smoke.ts` (uses a fresh fixture id each run).

## Remaining

Mainnet (real-time L12) run. Also note fixture 18192996's **goal counts** are Merkle-authenticated
against the live feed, while its `Completed` match-status code is supplied by the resolver — the
proven rulebook fail-closes every non-`Completed` status to `Refund` (INV-1), so the downside is
bounded to a refund, but binding status to an authenticated feed field is the gap to close before
mainnet (`docs/AUDIT.md`).
