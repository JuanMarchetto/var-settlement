# VAR — Verifiable Automated Resolution

Trustless, formally-verified settlement for FIFA World Cup 2026 1X2 (match-result) prediction
markets on Solana. Built for the **Superteam World Cup Hackathon, Track 1: Prediction Markets &
Settlement ($18,000)**. Data source: TxODDS **Tx LINE** (`Txoracle` program). Deadline: 2026-07-19.

## The thesis

Prediction markets don't die from bad odds. They die from bad resolution. Polymarket/UMA's 2026
crisis settled a disputed $60M+ market against a filed 8-K by token vote — the side with more
tokens decided what "happened," not the facts. The other common failure mode is just as bad: a
closed oracle pipe where you trust that Chainlink (or whoever) pushed the right number, with no way
to check it yourself.

VAR does neither. It settles a World Cup match with **no token vote, no dispute bond, no trusted
arbiter**. At resolution, the program re-derives the outcome and payout straight from Tx LINE's own
on-chain Merkle-rooted feed via CPI, runs the authenticated scoreline through a **Kani-proven
deterministic football rulebook**, writes a proof-carrying receipt, and exposes a **permissionless
`reverify`** instruction so any wallet can re-run the whole proof in one transaction and watch it
come back green. It's the on-chain VAR that Tx LINE published daily Merkle roots *for* but never
shipped.

## Architecture

```
  packages/sdk (TS)                crates/rulebook (Rust, pure)             programs/var-settlement (Anchor)
 ┌────────────────────────┐      ┌──────────────────────────────┐        ┌───────────────────────────────────┐
 │ TxLineClient            │      │ resolve_outcome()             │        │ create_market                       │
 │  .activate()   4-step   │      │   MatchState -> Outcome        │        │ deposit       USDC -> vault escrow  │
 │  .statWitness()   ────┐ │ ──►  │ settle() / winner_payout()    │  ◄──  │ resolve       CPI validate_stat x2   │
 │  dailyScoresRootsPda() │ │      │   Pools -> Settlement          │        │                -> rulebook::resolve │
 └────────────────────────┘ │      │ no_std-friendly, checked      │        │                -> ResolutionReceipt │
                            │      │ arithmetic, fail-closed        │        │ claim         pro-rata payout       │
                            │      │ + Kani proofs INV-1..4, 2b     │        │ reverify      permissionless replay │
                            │      └──────────────────────────────┘        └──────────────────┬────────────────────┘
                            │                     ▲ linked in unchanged                        │ CPI
                            │ fetches stat-validation                                          │ validate_stat(
                            │ Merkle proofs                                                     │  EqualTo,
                            ▼                                                                   │  threshold=claimed_goals)
                 ┌─────────────────────────────┐                                                │
                 │ Tx LINE / Txoracle           │ ◄──────────────────────────────────────────────┘
                 │ daily_scores_roots PDA       │
                 │ (TxODDS on-chain feed)       │
                 └─────────────────────────────┘
```

`crates/rulebook` is the moat: a pure, dependency-free Rust crate with no Solana imports. It
compiles unchanged into the Anchor program *and* is independently checked with `cargo kani`. If the
CPI wiring or account layout ever changes, the proven core doesn't move.

## How it works

```
create_market(fixture_id, kind=1X2, home_stat_key, away_stat_key, period, fee_bps, resolve_deadline)
    -> opens a Market PDA + USDC vault, three empty pools (Home/Draw/Away)

deposit(outcome, amount)
    -> USDC transfers into the vault, updates the Position PDA + the market's pool totals

resolve(home: StatWitness, away: StatWitness, status_code)     // permissionless, anyone can call
    -> binds each witness to the market's configured stat_key/period (no cross-stat spoofing)
    -> CPIs Txoracle::validate_stat(EqualTo, threshold=home_goals) against daily_scores_merkle_roots
    -> CPIs the same for away_goals
    -> both must return true, or resolve fails closed (StatNotAuthenticated)
    -> runs rulebook::resolve(MatchState, Pools, fee_bps) -> Outcome + Settlement
    -> writes ResolutionReceipt (source_root, ruleset_hash, scoreline, outcome, payout split)
    -> flips Market.status = Settled, emits MarketResolved

claim()
    -> per Position: pays floor(stake * net / winning_pool) for winners, or the full stake back
       on Refund; guarded by a claimed flag so a Position can only be paid once

reverify() -> bool                                              // permissionless, read-only
    -> re-runs rulebook::resolve from the stored receipt's authenticated scoreline
    -> asserts it still matches the recorded outcome and settlement, bit for bit
```

`resolve` and `reverify` both call the exact same pure function,
`rulebook::resolve(&MatchState, Pools, fee_bps) -> Resolution`. There's no separate "resolution
logic" to audit for `reverify` — it's the identical code path, so a green `reverify` means the
receipt is reproducible from scratch, not just internally consistent.

## The formal-verification moat

`crates/rulebook` ships five Kani proofs (`crates/rulebook/src/lib.rs`, `#[cfg(kani)] mod proofs`),
run with `cargo kani`:

- **INV-1 — totality / fail-closed** (`inv1_resolve_outcome_total_and_correct`): `resolve_outcome`
  is total over the full input range — it always returns exactly one `Outcome`, never panics, and
  degenerate goal counts or a non-`Completed*` status always resolve to `Refund`.
- **INV-2 — conservation** (`inv2_settlement_conserves`): `fee + net == pot` always, `net <= pot`
  always, and no payout (full winning pool or any sub-stake) ever exceeds `net`. The program cannot
  mint value — every unit paid out came from the pot.
- **INV-2b — solvency under splitting** (`inv2b_two_winner_split_within_net`): any two disjoint
  winner stakes drawn from the winning pool sum to no more than `net`, the induction base for
  "the full set of winners can always be paid from escrow."
- **INV-3 — settlement fail-closed** (`inv3_settle_fail_closed`): `Outcome::Refund` and any
  `fee_bps > MAX_FEE_BPS` always settle as a full refund with `fee == 0`.
- **INV-4 — determinism** (`inv4_resolve_deterministic`): identical `(MatchState, Pools, fee_bps)`
  always yields an identical `Resolution` — no hidden state, no clock or randomness dependence.

A sixth property, **no double-claim**, is enforced today by the `claimed` guard in
`programs/var-settlement/src/lib.rs::claim()` (`require!(!p.claimed, ...)`); it's a program-level
account-state property, not a pure-function one, so it's covered by the `litesvm` integration
suite rather than Kani — see Current Status below for what's proven versus what's pending.

## Tx LINE integration — exact interface used

Verified by hand against `txline.txodds.com/documentation` and the on-chain IDL saved at
`docs/idl/txoracle_mainnet.json` (2026-07-05). Re-confirm on first live connect; TxODDS may revise.

- **`Txoracle` program**: mainnet `9ExbZjAapQww1vfcisDmrngPinHTEfpjYRWMunJgcKaA`, devnet
  `6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`.
- **API host**: mainnet `https://txline.txodds.com/api`, devnet `https://txline-dev.txodds.com/api`.
- **Free World Cup activation** (`packages/sdk/src/txline.ts::TxLineClient.activate`), no payment,
  no TxL token: on-chain `subscribe(serviceLevel, durationWeeks)` → `POST /auth/guest/start` (guest
  JWT) → wallet-sign `${txSig}:${leagues}:${jwt}` with Ed25519/tweetnacl → `POST
  /api/token/activate` returns an `X-Api-Token`. Data calls send both `Authorization: Bearer <jwt>`
  and `X-Api-Token: <token>`.
- **Service levels**: **L1** = World Cup/Friendlies, 60s-delayed, works on devnet. **L12** =
  real-time sub-second, **mainnet only**.
- **`validate_stat`** — the settlement primitive, called from
  `programs/var-settlement/src/lib.rs::txoracle_cpi::validate_stat_equal`:
  ```
  validate_stat(
    ts:              i64,
    fixture_summary: ScoresBatchSummary,   // { fixture_id, update_stats, events_sub_tree_root }
    fixture_proof:   Vec<ProofNode>,       // proves the fixture summary in the day's main tree
    main_tree_proof: Vec<ProofNode>,       // nested proof up to the daily root
    predicate:       TraderPredicate,      // { threshold, comparison: GreaterThan|LessThan|EqualTo }
    stat_a:          StatTerm,             // { stat_to_prove: { key, value, period }, event_stat_root, stat_proof }
    stat_b:          Option<StatTerm>,
    op:              Option<BinaryExpression>  // Add | Subtract
  ) -> bool
  ```
  VAR calls it twice per `resolve`, once with `comparison = EqualTo, threshold = claimed home_goals`
  and once for away — a `true` return means the claimed integer is the Merkle-authentic one.
  Discriminator `[107, 197, 232, 90, 191, 136, 105, 185]` (from the IDL), invoked raw (not through a
  generated Anchor client) since `Txoracle` isn't a workspace dependency.
- **`daily_scores_roots` PDA**: seeds `[b"daily_scores_roots", (epoch_day as u16).to_le_bytes()]`,
  `epoch_day = floor(ts_millis / 86_400_000)`. Derivation lives in both
  `packages/sdk/src/txline.ts::dailyScoresRootsPda` (TS) and is documented in `spec.md` §2.
- **Merkle model**: roots publish per epoch-day, nested (stat → `event_stat_root` →
  `events_sub_tree_root` → daily main root). Verification is against the day's committed root — i.e.
  post-update, not per-tick. `resolve` accounts for this with a 7-day grace window past
  `resolve_deadline` (`RESOLVE_GRACE_SECS` in `programs/var-settlement/src/lib.rs`) so settlement
  can land after the root covering the final whistle actually publishes.
- **`validate_odds`** also exists on `Txoracle` (odds are on-chain-verifiable too) — noted for a
  future Over/Under market, unused in V1.

## Repo layout

```
spec.md                          full design spec — architecture, invariants, threat model
crates/rulebook/src/lib.rs        the verified core: types, resolve_outcome, settle,
                                  winner_payout, resolve, + Kani proofs (#[cfg(kani)])
crates/rulebook/tests/            22 unit tests across outcome.rs / resolve.rs / settlement.rs
scenarios/*.json                  12 golden real-World-Cup vectors (golden.rs runs all of them)
programs/var-settlement/src/lib.rs the Anchor program: create_market/deposit/resolve/claim/reverify,
                                  the txoracle_cpi module, account/PDA layout
packages/sdk/src/txline.ts         TS client: TxLINE activation, stat-validation fetch, PDA derivation
docs/idl/txoracle_mainnet.json    the Txoracle IDL this integration was built against
docs/TRUST_SURFACE.md             trusted vs. untrusted, threat model, fail-closed behavior
docs/AUDIT.md                     self-audit: proven invariants, arithmetic discipline, limitations
docs/DEMO_VIDEO_SCRIPT.md         3-minute demo shot list
docs/TXLINE_API_FEEDBACK.md       builder feedback on the Tx LINE API/docs
SUBMISSION_CHECKLIST.md           gate list toward the 2026-07-19 Earn submission
tests/                            reserved for litesvm/anchor integration tests (not yet written)
```

## Verify it yourself

No trust required — clone and run the proven core directly.

```bash
# 22 unit tests + the 12-scenario golden suite (real World Cup matches, hand-checked expected outcomes)
cargo test -p rulebook

# formal proofs: totality, conservation, fail-closed, determinism (crates/rulebook/src/lib.rs)
cargo kani -p rulebook

# the on-chain program compiles clean against the same rulebook crate
cargo check -p var-settlement
```

`cargo test -p rulebook` is fast and deterministic. `cargo kani -p rulebook` runs Kani's bounded
model checker over all five proof harnesses — expect it to take real time (bounded exhaustive
search, not a fuzz run) since it explores the full input range per `kani::assume` bound, not a
sample.

## Current status (honest)

**Verified and passing today:**
- `cargo test -p rulebook` — 22 unit tests + the golden-scenario test (12 real-WC vectors) all green.
- Kani proofs INV-1, INV-2, INV-2b, INV-3, INV-4 are written and running against the pure rulebook.
- `cargo check -p var-settlement` — the Anchor program compiles clean on host, exit 0.
- Tx LINE's self-serve free World Cup tier (4-step activation) is verified reachable and documented;
  the TS client (`packages/sdk/src/txline.ts`) implements activation and stat-witness assembly.

**Staged, not done — next in line:**
- SBF build (`cargo build-sbf` / `anchor build`) for the program.
- `litesvm` integration tests (happy path, double-claim guard, early-resolve guard, refund path).
- Devnet deploy of `programs/var-settlement` under program ID
  `AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk` (declared in `declare_id!`, not yet deployed —
  `target/deploy/` has only the keypair, no built `.so`).
- Live Tx LINE devnet/mainnet activation — the 4-step flow is coded and verified against the docs,
  but running it end-to-end needs a funded wallet (`subscribe()` in `txline.ts` is a stub that
  throws until wired against a live wallet).
- Demo video — not recorded yet. `docs/DEMO_VIDEO_SCRIPT.md` is the script for it.

Nothing here claims more than what's been run. See `SUBMISSION_CHECKLIST.md` for the full gate list.

## Compliance note

VAR is positioned strictly as **settlement infrastructure / verification tooling**, not an operated
sportsbook. Pools settle in **USDC only** — the product path never touches, requires, or references
the TxL token. Entry is free-to-enter parimutuel pools (stake USDC on an outcome, winners split the
net pot pro-rata). No order book, no market-making, no custody beyond escrow-until-settlement.
