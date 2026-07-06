# VAR — Verifiable Automated Resolution

**Project name:** VAR (Verifiable Automated Resolution)

**Tagline:** Trustless, formally-verified settlement for World Cup 2026 match-result markets — no token vote, no dispute bond, no arbiter.

**Track:** Prediction Markets & Settlement ($18,000)

> **Proof it's real (open these first):**
> - **Live real-feed settlement — `resolve` tx on devnet:** https://explorer.solana.com/tx/4j2ukzmW8rJNMAuCiyyKaiqksviB6mZS26e4FSfFuhBynV5mQXv8DMasLJUopv3XUC4BsFHQxPNPLPoj69oVtnyC?cluster=devnet — outcome **Away**, `reverify() → true`, winner paid pro-rata.
> - **4 Kani formal proofs, all PASS.** Re-run in one command: `cd crates/rulebook && cargo kani`.

---

## Elevator pitch

Prediction markets don't die from bad odds — they die from bad resolution. VAR settles FIFA World Cup 2026 1X2 (Home/Draw/Away) markets by CPI'ing into TxODDS Tx LINE's `validate_stat` to authenticate the final **goal counts** against the on-chain daily Merkle root, then runs them through a **Kani-proven deterministic rulebook** that writes a proof-carrying receipt. Anyone can call a permissionless `reverify` — a single read-only call, no signer, no fee — and re-derive the entire resolution, so the settlement is machine-checked and independently re-verifiable: no human, token, or arbiter can override it.

---

## The problem

Resolution is the single point of failure for every prediction market, and both incumbent designs fail it:

- **Token-vote arbitration.** Polymarket/UMA's 2026 dispute crisis resolved a reported nine-figure contested market *against a filed 8-K* by plutocratic token vote — the side holding more tokens decided what "happened," not the facts. Resolution became a governance attack surface.
- **Closed oracle pipes.** The alternative is to trust that Chainlink (or whoever) pushed the right number into a feed you cannot independently re-check. You get a receipt, not a proof.

Both ask you to *trust*. Neither lets you *verify*. For a market of any size, "trust the token holders" and "trust the pipe" are not settlement — they're liabilities.

---

## The solution — how it works

VAR is a settlement engine, not a book. The full lifecycle:

1. **`create_market`** — opens a Market PDA + USDC vault with three empty parimutuel pools (Home/Draw/Away), binding the fixture's home/away goal stat-keys, period, fee, and resolve deadline at creation.
2. **`deposit`** — USDC transfers into the escrow vault; the depositor's Position PDA and the market's pool totals update.
3. **`attest_home` + `resolve` (two-step)** — permissionless. Each step CPIs into Tx LINE's `Txoracle::validate_stat` (`comparison = EqualTo`, `threshold = claimed goals`) to authenticate the home and away final-goal counts against the on-chain daily Merkle root. Both must return `true` or settlement **fails closed** (`StatNotAuthenticated`). It's split into two transactions because both Merkle proofs together exceed Solana's 1232-byte tx limit. On success, the Kani-proven `rulebook` maps `(home_goals, away_goals, status)` → `Outcome` + payout split and writes a `ResolutionReceipt` (source root, ruleset hash, scoreline, outcome, split).
   - **What the CPI proves — and what it doesn't.** The two goal counts are Merkle-authenticated on-chain. The match-status code is a *caller-supplied input*, not an oracle-authenticated field. This is safe because the proven rulebook fail-closes every non-`Completed` status to `Refund` (INV-1): a caller cannot manufacture a decisive result from an unfinished match — the worst they can force is a refund. Binding status to an authenticated feed field is a follow-up (see *Honest status*).
4. **`reverify` → bool** — permissionless, read-only. A single `.view()` call — **no signer, no fee, no state change** — that re-runs the *exact same* pure `rulebook::resolve` from the stored receipt and asserts it still matches the recorded outcome bit-for-bit. A green `reverify` means the receipt is reproducible from scratch by *any* wallet (or none), not just internally consistent.
5. **`claim`** — pro-rata payout: winners receive `floor(stake * net / winning_pool)`, refunds return the full stake, guarded by a `claimed` flag so a Position is paid at most once.

---

## Why it's different — the moat

The resolution core is a pure, dependency-free Rust crate (`crates/rulebook`) with no Solana imports. It compiles unchanged into the Anchor program **and** is independently model-checked with `cargo kani`. The formal-verification guarantee is **decoupled from the proof transport** — if the CPI wiring or account layout changes, the proven core doesn't move.

This maps directly onto what the flagship track calls "highly valued":

- **A custom Merkle verification layer built on Tx LINE's primitive** — VAR authenticates each goal count against the on-chain `daily_scores_roots` PDA through the nested stat → `event_stat_root` → `events_sub_tree_root` → daily-root proof chain, with anti-spoofing stat-key/period binding fixed at market creation.
- **A settlement engine that CPIs into `validate_stat`** — twice per resolution, once per goal stat, both required to return `true`.
- **Deterministic, well-documented resolution code** — and here the determinism is **machine-proven with Kani, not merely asserted.** The rulebook's totality, value-conservation, fail-closed, and determinism invariants are model-checked, not just unit-tested.

No token vote. No dispute bond. No trusted arbiter. No admin override. Correct-by-construction resolution is the design goal: remove the dispute layer instead of bolting governance on top of it.

---

## How Tx LINE is used

- **Real on-chain feed.** Settlement authenticates against Tx LINE's `Txoracle` daily Merkle roots — the `daily_scores_roots` PDA (seeds `[b"daily_scores_roots", (epoch_day as u16).to_le_bytes()]`). Devnet `Txoracle`: `6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`.
- **`validate_stat` is the settlement primitive.** It's a predicate checker returning `bool`, not a "read the score" call — so VAR both *knows* the claimed goals (fetched off-chain with their `stat-validation` Merkle proofs) and *authenticates* them via CPI (`EqualTo, threshold = claimed_goals`). A `true` return means the claimed integer is the Merkle-authentic one. Invoked raw against the IDL discriminator since `Txoracle` isn't a workspace dependency.
- **Free 4-step World Cup activation, live.** No payment, no TxL token: on-chain `subscribe(1, 4)` → `POST /auth/guest/start` (guest JWT) → wallet-signed `${txSig}:${leagues}:${jwt}` (Ed25519/tweetnacl) → `POST /api/token/activate` (X-Api-Token). Runs on the L1 World Cup devnet tier. Verified end-to-end (activation tx `2hnw1aAkGN4RRqfzRyJiDUEKCq1BnuH9Wm7X6vRet2ozvcZJy1ngU16Fu7NBHoV3rpmKWUdRs1PYgJ2c1C5w778C`).

---

## What's built & proven

Every claim below is backed by evidence in the repo — clone it and re-run.

- **4 Kani formal proofs, all PASS** (`cd crates/rulebook && cargo kani`; transcript at `docs/KANI_PROOF_TRANSCRIPT.txt`, `Complete - 4 successfully verified harnesses, 0 failures`):
  - **INV-1** totality / fail-closed — `resolve_outcome` is total, never panics, and every non-`Completed` / degenerate state resolves to `Refund`.
  - **INV-2** fee conservation — `fee + net == pot`, `net ≤ pot`, no payout ever exceeds `net`. The program cannot mint value.
  - **INV-3** settlement fail-closed — `Refund` and any out-of-range `fee_bps` always settle as a zero-fee full refund.
  - **INV-4** determinism — identical inputs always yield an identical resolution; no hidden state, clock, or randomness.
- **Test suite** (`cargo test -p rulebook`): 25 unit tests + 12 golden real-World-Cup vectors (Argentina–France 2022 penalties → **Draw** on the 90' scoreline, VAR-disallowed goal, abandonment/postponement/void → Refund, zero-winning-pool refund) + 12,000 proptest cases (3 payout/solvency properties × 4,000 cases each, at full USDC magnitude).
- **Deployed on devnet:** `var_settlement` = `AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk` (352KB, upgraded, upgrade authority = builder wallet).
- **Live settlement against the real feed (devnet):** real fixture **18192996** (feed score home **2 – 3** away), authenticated via live Tx LINE `stat-validation` Merkle proofs and resolved by two-step CPI into the **real** `Txoracle` over the on-chain daily root (needs a 1.4M compute-unit budget — Merkle verification is CU-heavy). The two **goal counts** are Merkle-authenticated on-chain; the `Completed` match-status code is supplied by the resolver and fail-closed by the proven rulebook. Result: outcome **Away**, `reverify() → true`, winning pool paid pro-rata.
  - `attest_home` tx: `53dkuaseF6pAD71WDAaPUzwEQFQ6keWgRuafVM8DBqyvBZqWMwQg3GAtzbuJP5fSFYJ1rxpDKbE7HMK1AXtfbsws`
  - `resolve` tx: `4j2ukzmW8rJNMAuCiyyKaiqksviB6mZS26e4FSfFuhBynV5mQXv8DMasLJUopv3XUC4BsFHQxPNPLPoj69oVtnyC`
- **End-to-end smoke test PASSES** (`tests-devnet/smoke.ts`): full `create → deposit → resolve → reverify → claim` lifecycle with real SPL transfers; receipt outcome `Home`, `reverify() → true`, final balances **158 / 40 / 2** (the 2% protocol fee), exactly the rulebook's settlement math. (Uses a mock `Txoracle` — test-only devnet `85KwDRzyZeG8wAXVCZo2CKTVor3qVcyhq7vk2yAzBJMw`, never used on mainnet.)

---

## Links

- **GitHub repo:** https://github.com/JuanMarchetto/var-settlement
- **Live `resolve` tx (Solana Explorer, devnet):** https://explorer.solana.com/tx/4j2ukzmW8rJNMAuCiyyKaiqksviB6mZS26e4FSfFuhBynV5mQXv8DMasLJUopv3XUC4BsFHQxPNPLPoj69oVtnyC?cluster=devnet
- **Program (Solana Explorer, devnet):** https://explorer.solana.com/address/AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk?cluster=devnet
- **Demo video:** _[placeholder — link to be added]_
- **Verify it yourself:** `cargo test -p rulebook` · `cd crates/rulebook && cargo kani`

---

## Tech stack

Rust · Anchor 0.32 · **Kani 0.67** (formal verification / bounded model checking) · Solana · SPL parimutuel USDC escrow · TypeScript SDK (`@coral-xyz/anchor`, `tweetnacl`) · TxODDS **Tx LINE** `Txoracle` on-chain daily Merkle feed. Architecture: a pure, dependency-free Kani-proven `rulebook` crate that links unchanged into the Anchor program and is independently model-checked; two-step `resolve` because both Merkle proofs exceed the 1232-byte tx limit.

---

## Honest status & next steps

- **Devnet is the target and it's sufficient** — mainnet is not a hackathon requirement, and Tx LINE's free devnet L1 tier is what the World Cup access grants. The settlement *proof* against the real on-chain feed is what's real; the demo's USDC is a test mint standing in for USDC.
- **Optional mainnet L12** (real-time sub-second, mainnet-only) is a straight wiring follow-up — the same 4-step activation and CPI path, no core change.
- **V1 is one market family (1X2 match-result).** The `MatchState`/`Outcome` types are designed to extend to Over/Under and correct-score — Tx LINE also exposes a `validate_odds` primitive for those, noted for future markets — but V1 ships 1X2 only.
- **One open soundness item, flagged not hidden** (full self-audit in `docs/AUDIT.md`, trust surface in `docs/TRUST_SURFACE.md`): **match status is a resolver input, not oracle-authenticated.** `validate_stat` proves the goal counts against the Merkle root; the match-status code is supplied by the caller. The proven rulebook fail-closes every non-`Completed` status to `Refund` (INV-1), so the downside is bounded to a refund — never a fabricated decisive result — but binding status to an authenticated feed field is a gap to close before mainnet. (Both goal witnesses *are* bound on-chain to the market's own `fixture_id`, stat-key, and period — `FixtureMismatch` / `StatKeyMismatch` / `StatPeriodMismatch` — so a cross-fixture or cross-stat proof is rejected.)

---

## Compliance

VAR is settlement infrastructure / verification tooling — not an operated sportsbook. Pools settle in **USDC only**; the product path never touches, requires, or references the TxL token. Free-to-enter parimutuel pools, no order book, no market-making, and **non-custodial beyond deterministic on-chain escrow** — the vault PDA holds USDC under program rules until settlement, with no admin withdrawal path.
