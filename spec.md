# VAR — Verifiable Automated Resolution

**Trustless, formally-verified settlement for FIFA World Cup 2026 outcome markets on Solana.**

Superteam World Cup Hackathon — Track 1: *Prediction Markets & Settlement* ($18,000).
Data: TxODDS **Tx LINE** (`Txoracle` program). Deadline: 2026-07-19.

---

## 0. One-paragraph thesis

Prediction markets live and die on **resolution**. Polymarket/UMA's 2026 crisis (a disputed
$60M+ market resolved against a filed 8-K by token vote) and closed oracle pipes ("trust that
Chainlink pushed the right number") are the incumbent pain. VAR settles a World Cup market with
**no token vote, no dispute bond, no trusted arbiter**: at resolution the program re-derives the
payout from TxLINE's own on-chain Merkle-rooted feed through a **Kani-proven deterministic football
rulebook**, writes a proof-carrying receipt, and exposes a **permissionless `reverify`** instruction
so any wallet re-runs the entire proof in one transaction and watches it come back green. It is the
on-chain VAR that Tx LINE published daily Merkle roots *for* but never shipped.

The moat is the exact intersection the flagship rubric calls "highly valued": a **custom verification
layer built on TxLINE's Merkle-proof primitive** + a **settlement engine that CPIs into
`validate_stat`** + **deterministic, well-documented resolution code** — here, that determinism is
*machine-proven* with Kani, which almost no hackathon team ships.

---

## 1. Scope (ruthlessly narrowed for a ~13-day solo sprint)

### In scope (V1)
- **One market family: 1X2 "match result"** (Home / Draw / Away) on the **regulation 90' result**
  (the standard football-market convention, so knockout ties that go to extra time / penalties still
  settle **Draw** on the 90' scoreline). This single market family exercises every hard edge case
  (regulation vs ET/penalties, VAR goal reversals, abandonment) while keeping the surface minimal.
- **Parimutuel USDC escrow**: three outcome pools; winners split the total pot pro-rata minus a fixed
  protocol fee (basis points). No order book, no AMM, no counterparty risk — pure pool settlement.
- **Trustless resolution** via CPI into `Txoracle::validate_stat` against the on-chain
  `daily_scores_roots` PDA. The program authenticates the final home/away goals, then the Kani-proven
  `rulebook` maps `(home, away, status)` → `Outcome` + payout split.
- **Proof-carrying `ResolutionReceipt`**: source root hash, ruleset hash, authenticated scoreline,
  outcome, payout split, timestamp.
- **Permissionless `reverify`**: re-runs rulebook against the stored receipt and the on-chain root and
  asserts equality. The demo centerpiece.

### Out of scope (V1) — explicitly non-goals
- Over/Under, to-qualify, correct-score, live in-play markets (rulebook is designed to extend to them; V1 ships 1X2 only).
- An operated sportsbook, market-making, or any custody beyond escrow-until-settlement.
- The TxL token. **Settlement is USDC only.** Never touch/require the TxL token in the product path.
- A dispute/governance layer. The whole point is that correct resolution needs no dispute.

### Descope floor (if behind by ~Jul 15)
1. Drop mainnet L12; run entirely on **devnet L1** (60s-delayed) + an explicitly-allowed **simulated
   replay** of a real match's score sequence. Everything stays verifiable; only the feed is delayed.
2. Drop the Next.js viewer; ship a **CLI `reverify`** + Solana Explorer links.
3. Keep, non-negotiable: the Kani-proven rulebook, the on-chain resolve/claim/reverify path, and one
   real settled market. That alone clears the "working, verifiable settlement" bar.

---

## 2. Ground truth — verified Tx LINE interface (2026-07-05, from live docs + on-chain IDL)

> Source: `txline.txodds.com/documentation/*` and the `Txoracle` IDL saved at
> `docs/idl/txoracle_mainnet.json`. Verified by hand, not summarized. Re-confirm against live docs on
> first connect; TxODDS may revise.

- **Program (`Txoracle`)**: mainnet `9ExbZjAapQww1vfcisDmrngPinHTEfpjYRWMunJgcKaA`,
  devnet `6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`.
- **API host**: mainnet `https://txline.txodds.com/api`, devnet `https://txline-dev.txodds.com/api`.
- **Free World Cup access** (no payment, no TxL): 4-step self-serve activation — on-chain
  `subscribe(serviceLevel, durationWeeks)` → `POST /auth/guest/start` (guest JWT) → wallet-sign
  `${txSig}:${leagues}:${jwt}` (Ed25519 / tweetnacl) → `POST /api/token/activate` (X-Api-Token).
  Data calls send `Authorization: Bearer <jwt>` + `X-Api-Token: <token>`.
- **Service levels**: **L1** = World Cup/Friendlies **60s-delayed** (works on devnet);
  **L12** = **real-time sub-second, MAINNET-ONLY**.
- **`validate_stat` (the settlement primitive — exact signature from IDL):**
  ```
  validate_stat(
    ts:              i64,
    fixture_summary: ScoresBatchSummary,     // { fixture_id:i64, update_stats:ScoresUpdateStats, events_sub_tree_root:[u8;32] }
    fixture_proof:   Vec<ProofNode>,         // proves fixture summary in the day's main tree
    main_tree_proof: Vec<ProofNode>,         // (nested) proof to the daily root
    predicate:       TraderPredicate,        // { threshold:i32, comparison: GreaterThan|LessThan|EqualTo }
    stat_a:          StatTerm,               // { stat_to_prove:ScoreStat{key:u32,value:i32,period:i32}, event_stat_root:[u8;32], stat_proof:Vec<ProofNode> }
    stat_b:          Option<StatTerm>,
    op:              Option<BinaryExpression> // Add | Subtract  (combine stat_a, stat_b before comparing to threshold)
  ) -> bool
  ```
  - **Accounts:** `daily_scores_merkle_roots` (the PDA).
  - **PDA derivation:** seeds `[b"daily_scores_roots", (epoch_day as u16).to_le_bytes()]`,
    `epoch_day = floor(ts_millis / 86_400_000)`. (Verified: `BN(epochDay).toArrayLike("le", 2)` in docs.)
  - **Returns `bool`**: `true` iff the (proven-authentic) `stat_a [op stat_b]` satisfies
    `comparison threshold`. Errors `StatKeyMismatch` / `PredicateFailed` on bad input.
- **`validate_odds`** also exists (odds are on-chain-verifiable too) — noted for future O/U markets; unused in V1.
- **Merkle model:** roots are published **per epoch-day** into the PDA (nested tree: stat →
  `event_stat_root` → fixture `events_sub_tree_root` → daily main root). Verification is therefore
  against the day's committed root, i.e. **post-update, not per-tick**. Settlement must occur after
  the root covering the final-whistle update is published (minutes-to-same-day, not necessarily a full day).

### Design consequence (the key architectural decision)
`validate_stat` is a **predicate checker returning bool**, not a "read the score" call. To settle we
must both **know** and **authenticate** the final goals. Flow:

1. Off-chain SDK fetches the final scoreline + `stat-validation` proofs for the home-goals and
   away-goals stats (`GET /api/scores/stat-validation?fixtureId&seq&statKey`).
2. `resolve` receives claimed `(home_goals, away_goals)` + the proofs from any caller (permissionless).
3. `resolve` **CPIs `validate_stat` with `comparison = EqualTo, threshold = home_goals`** for the
   home stat, and again for away. Both must return `true` → the claimed scoreline is Merkle-authentic.
4. The **Kani-proven `rulebook`** maps authenticated `(home, away, status)` → `Outcome` + payout split.
5. `resolve` writes the `ResolutionReceipt` and flips the market to `Settled`.

The `rulebook` operates only on authenticated integers, so **the formal-verification moat is fully
decoupled from the proof transport** — if CPI details shift, the proven core is unaffected.

---

## 3. Architecture

```
packages/sdk (TS)          crates/rulebook (Rust, pure, Kani-proven)      programs/var-settlement (Anchor)
  TxLINE activation    ┐        MatchState -> Outcome -> PayoutSplit    ┌─ create_market
  REST/SSE ingest      ├──►  (dependency-free, no_std-friendly core) ◄──┤   deposit  (USDC escrow)
  stat-validation      │        + Kani proofs (conservation,             │   resolve  (CPI validate_stat x2 -> rulebook -> receipt)
  merkle proof fetch   ┘         determinism, totality, fail-closed)     │   claim    (pro-rata payout)
                                                                         └─ reverify (permissionless re-derivation)
```

- **`crates/rulebook`** — the moat. Pure Rust, no Solana/std-only deps, checked arithmetic, fail-closed.
  Compiled into the program AND independently Kani-verified. Ships the golden edge-case suite.
- **`programs/var-settlement`** — Anchor 0.32 program. Owns escrow + CPI + receipts. Depends on `rulebook`.
- **`packages/sdk`** — TS: TxLINE activation, snapshot/stream ingest, proof assembly, `reverify` client.

### Account model (PDAs, program-owned)
| PDA | Seeds | Holds |
|-----|-------|-------|
| `Market` | `[b"market", fixture_id_le, market_kind]` | fixture id, kind, status, statKey/period config, pool totals per outcome, fee_bps, resolution deadline, `ResolutionReceipt` (once settled) |
| `Vault` (USDC ATA) | `[b"vault", market]` | escrowed USDC; authority = market PDA |
| `Position` | `[b"position", market, owner]` | owner, per-outcome stake, claimed flag |

### `ResolutionReceipt` (written by `resolve`, read by `reverify`)
```
{ source_root: [u8;32],      // the daily_scores_roots value used
  ruleset_hash: [u8;32],     // hash of rulebook version + market config (binds the rules applied)
  home_goals: i32, away_goals: i32, status: MatchStatus,
  outcome: Outcome, payout_bps: [u16;3],  // Home/Draw/Away pool weighting after settlement
  resolved_ts: i64 }
```

---

## 4. The rulebook state machine (what Kani proves)

### Types
```
enum MatchStatus { Completed, CompletedAfterExtraTime, CompletedAfterPenalties, Abandoned, Postponed, Void }
enum Outcome     { Home, Draw, Away, Refund }          // Refund => VOID market, stakes returned
struct MatchState { home_goals: i32, away_goals: i32, status: MatchStatus }  // goals = REGULATION 90' goals
```

### Resolution rules (1X2, regulation convention) — deterministic, total
1. `status ∈ {Abandoned, Postponed, Void}` → **`Refund`** (fail-closed to VOID; stakes returned).
2. Any `Completed*` status → compare **regulation** `home_goals` vs `away_goals`:
   `>` → `Home`, `<` → `Away`, `==` → **`Draw`** (even when the tie was decided by ET/penalties —
   the match-result market settles on 90').
3. Negative goals, or an unrecognized/degenerate state → **`Refund`** (never panic, never mis-award).

### Payout (parimutuel)
- Let `pot = pool[Home]+pool[Draw]+pool[Away]`, `fee = pot * fee_bps / 10_000`, `net = pot - fee`.
- Winning pool `W = pool[outcome]`. A winner with stake `s` in the winning outcome claims
  `floor(s * net / W)` (integer math; dust ≤ number-of-winners stays in vault, provably not minted).
- `Refund`: every depositor reclaims exactly their own total stake; `fee = 0`.
- Degenerate `W == 0` on a decisive outcome (nobody backed the winner) → treat as `Refund` (fail-closed).

### Kani invariants (proven on the pure core)
- **INV-1 Determinism/totality:** `resolve_outcome` is total — for every `MatchState` in range it
  returns exactly one `Outcome`, never panics, never overflows (checked arithmetic).
- **INV-2 Conservation:** `sum(payouts) + fee + dust == pot`, and `sum(payouts) ≤ net`. The program
  never pays out more than escrowed (no mint) under any pool configuration in range.
- **INV-3 Fail-closed:** any non-`Completed*` status or degenerate input yields `Refund`, and `Refund`
  returns each depositor exactly their stake (`sum(refunds) == pot`, `fee == 0`).
- **INV-4 Monotonic determinism:** identical `(MatchState, pools, fee_bps)` always yields identical
  `(Outcome, payouts)` — no hidden state, no time/randomness dependence.
- **INV-5 No double-claim (program-level, litesvm):** a `Position` can be claimed at most once.

### Golden edge-case suite (`scenarios/*.json`, real WC history)
Each scenario: `MatchState` + pools + `fee_bps` → expected `Outcome` + expected payouts. Includes:
1. Ordinary decisive win (2-0).  2. **90' draw taken to penalties** (e.g. a knockout shootout) → **Draw**.
3. **VAR goal disallowed at 90+5** (goal reversed → different regulation score).  4. **Abandonment after 80'** → Refund.
5. Own-goal / stat correction changing the winner.  6. Nobody backed the winner (`W==0`) → Refund.
7. Fee rounding / dust boundary.  8. Extra-time winner but 90' was a draw → **Draw**.

---

## 5. Settlement & verification flow

```
create_market(fixture_id, kind=1X2, home_stat_key, away_stat_key, period, fee_bps, resolve_deadline)
deposit(outcome, amount)                      // USDC -> Vault, updates Position + pool totals
resolve(ts, home_goals, away_goals, status,   // permissionless
        home_summary, home_fixture_proof, home_main_proof, home_stat_term,
        away_summary, away_fixture_proof, away_main_proof, away_stat_term):
    require status configured; require now <= resolve window
    CPI validate_stat(EqualTo, threshold=home_goals, stat_a=home_stat_term, ...) == true
    CPI validate_stat(EqualTo, threshold=away_goals, stat_a=away_stat_term, ...) == true
    outcome, payout_bps = rulebook::resolve(MatchState{home_goals,away_goals,status}, pools, fee_bps)
    write ResolutionReceipt; status = Settled
claim():                                       // per winner / per refund
    require Settled and not position.claimed
    transfer floor(stake * net / W) (or stake if Refund) from Vault; position.claimed = true
reverify() -> bool:                            // permissionless, read-only (.view)
    recompute rulebook::resolve from receipt's authenticated scoreline; assert == stored outcome/split
    (optionally re-CPI validate_stat to re-anchor against the live on-chain root)
```

---

## 6. Threat model / trust surface
- **Trusted:** TxODDS as the *source of truth* for match facts (unavoidable — it is the data sponsor).
  VAR's claim is narrower and honest: *given* TxLINE's signed feed, settlement is **correct,
  deterministic, and independently re-checkable** — no human/token/arbiter can override it, and anyone
  can re-run the proof. That is strictly more than "trust the oracle + a receipt".
- **Untrusted:** the `resolve` caller (permissionless) — cannot lie about the score because both goal
  stats must pass `validate_stat` against the on-chain root; cannot resolve early (deadline); cannot
  double-pay (conservation proof + claimed flag).
- **Fail-closed everywhere:** missing/degenerate data → Refund, never a wrong award.

## 7. Test strategy
- **Rulebook:** strict TDD (RED→GREEN→REFACTOR) unit tests per rule + golden scenarios; then **Kani**
  proofs INV-1..4. `cargo test` + `cargo kani` both green.
- **Program:** `litesvm`/`anchor test` — happy path (create→deposit→resolve-with-mocked-Txoracle→claim→
  reverify), double-claim guard, early-resolve guard, refund path. A local mock `Txoracle` stub returns
  configurable `validate_stat` bools for deterministic CI; a devnet run hits the real program.
- **CI gate:** `cargo test && cargo kani && anchor test` before every submission checkpoint.

## 8. Compliance framing
Positioned strictly as **settlement infrastructure / verification tooling**, settled in **USDC**,
never the TxL token, never an operated book. Free-to-enter parimutuel pools. KYC prepared early for an
Argentina payout.

## 9. Deliverables (submission gate)
Public GitHub repo (fresh, in-window git) · 5-min demo video · README with copy-paste "verify it
yourself" · tech doc listing exact TxLINE endpoints used · TRUST_SURFACE.md + AUDIT.md · TxLINE API
feedback note · `cargo kani` PASS transcript.
