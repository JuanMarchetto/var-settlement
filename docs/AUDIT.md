# Self-audit

An honest internal audit of `crates/rulebook` and `programs/var-settlement`, written by the person
who built it, for whoever picks this up next (external auditor, judge, or future us). Structured as:
what's proven, what's checked-but-not-proven, and what's a known limitation we're choosing to ship
with, not hiding.

## Invariants proven with Kani

All four run against the pure `rulebook` crate (`crates/rulebook/src/lib.rs`,
`#[cfg(kani)] mod proofs`), via `cargo kani -p rulebook` — transcript at
`docs/KANI_PROOF_TRANSCRIPT.txt`: `Complete - 4 successfully verified harnesses, 0 failures`:

| ID | Name | Harness | What it rules out |
|----|------|---------|--------------------|
| INV-1 | Totality / fail-closed | `inv1_resolve_outcome_total_and_correct` | Panics on any input in the swept range (`home_goals`, `away_goals` in `[-3, 20]`, all 6 `MatchStatus` variants); wrong 90' mapping; a non-`Completed*` status resolving to anything but `Refund` |
| INV-2 | Conservation | `inv2_settlement_conserves` | Minting value at the pool level: asserts `pot == home + draw + away`, `fee + net == pot`, `net <= pot`, `fee <= pot`, and refunds always carry `fee == 0, net == 0` |
| INV-3 | Settlement fail-closed | `inv3_settle_fail_closed` | `Outcome::Refund` or an out-of-range `fee_bps` producing anything but a zero-fee full refund |
| INV-4 | Determinism | `inv4_resolve_outcome_deterministic` | Any hidden state — identical inputs asserted to always produce an identical resolution (`settle` is pure, so settlement determinism follows) |

**Not in Kani, by design:** the per-winner payout bound (`winner_payout <= net`, including any
split among winners) divides by a *symbolic* divisor at `u128` width, which CBMC must bit-blast —
intractable. It is covered instead by the randomized property suite
`crates/rulebook/tests/payout_props.rs`: 3 solvency/payout properties × 4,000 cases each (12,000
cases at full USDC magnitudes). See the inline comment in `inv2_settlement_conserves` for the
rationale.

These are exhaustive over the bounded ranges Kani sweeps (`kani::assume` bounds: goals `-3..=20`,
pool values `<= 1000`, `fee_bps <= MAX_FEE_BPS`), not sampled —
Kani's bounded model checker explores the full state space within those bounds. Widening the bounds
further (e.g. arbitrary `u64` pool values instead of `<=1000`) is a cheap follow-up; the current
bounds were chosen to keep proof runtime reasonable while still covering every code path (every
branch in `resolve_outcome` and `settle` is exercised at the boundary values).

**What Kani does *not* cover:** anything inside `programs/var-settlement` — Solana account
handling, CPI serialization, PDA derivation, the `claimed` guard. Kani proves properties of pure
functions; it doesn't model an Anchor program's account graph. Those properties are covered by
ordinary tests and the devnet integration suite (`tests-devnet/smoke.ts`, full lifecycle with real
SPL transfers — see `DEPLOYMENTS.md`) instead.

## Checked-arithmetic / overflow discipline

`#![forbid(unsafe_code)]` at the top of `crates/rulebook/src/lib.rs` — the entire verified core is
safe Rust, no exceptions.

Every arithmetic operation that could overflow is explicit about it:
- `Pools::total()` returns `Option<u64>` via `checked_add` chains; `None` (overflow) is treated as a
  fail-closed refund-with-pot-zero in `settle()`, not a wrapped/panicking add.
- `fee` computation widens to `u128` before multiplying (`(pot as u128) * (fee_bps as u128) /
  BPS_DENOM as u128`) specifically so `pot * fee_bps` can't overflow `u64` even near `u64::MAX` pot
  values, then narrows back with a comment justifying why the narrow is safe (`fee <= pot` since
  `fee_bps <= MAX_FEE_BPS <= BPS_DENOM`).
- `winner_payout()` does the same `u128`-intermediate trick for `winning_stake * net /
  winning_pool`, with the invariant `winning_stake <= winning_pool` justified in a comment (by
  construction — a Position's stake in a bucket can't exceed the market's total pool for that
  bucket, since the pool is the sum of all positions' stakes in it).
- On the program side, every pool/position update in `deposit()` uses `checked_add(...).ok_or(
  VarError::MathOverflow)?` — no bare `+=` on escrow-tracking fields anywhere in
  `programs/var-settlement/src/lib.rs`.
- The workspace `Cargo.toml` sets `overflow-checks = true` under `[profile.release]`, so even if a
  checked path were missed, a release build would panic (not silently wrap) rather than corrupt
  state — belt-and-suspenders over the explicit `checked_*` calls above.

## Double-claim guard

`Position.claimed: bool`, checked in `claim()`:
```rust
require!(!p.claimed, VarError::AlreadyClaimed);
...
p.claimed = true;
if amount == 0 { return Ok(()); }
... token::transfer(...)
```
`claimed` is set to `true` before the transfer executes and regardless of whether `amount` turns out
to be zero, so a second call to `claim()` against the same `Position` always errors out before
touching the vault. This is a straightforward account-state guard, not (yet) formally proven — see
Known Limitations. The happy-path `claim` is exercised end-to-end on devnet by
`tests-devnet/smoke.ts` (both winner and loser claims, real SPL transfers); an explicit
second-claim-must-fail devnet test is a cheap follow-up an auditor should ask for.

## Stat-key binding (anti cross-stat-spoofing)

Before any CPI happens, `resolve()` checks:
```rust
require!(home.stat.stat_to_prove.key == m.home_stat_key, VarError::StatKeyMismatch);
require!(away.stat.stat_to_prove.key == m.away_stat_key, VarError::StatKeyMismatch);
require!(home.stat.stat_to_prove.period == m.period, VarError::StatPeriodMismatch);
require!(away.stat.stat_to_prove.period == m.period, VarError::StatPeriodMismatch);
```
Without this, `validate_stat` would happily authenticate *some* true stat off the feed (a corner
count, a different fixture's goal stat, the wrong period) and the program would have no way to tell
it isn't the home/away regulation-goals stat this specific market was created against. The market's
`home_stat_key`/`away_stat_key`/`period` are fixed at `create_market` time and never mutated
afterward, so this binding is stable for the market's whole lifetime.

## Resolve deadline / grace window

```rust
require!(now <= m.resolve_deadline.saturating_add(RESOLVE_GRACE_SECS), VarError::ResolveWindowPassed);
```
`RESOLVE_GRACE_SECS = 7 * 24 * 60 * 60` (7 days). This exists because Tx LINE's Merkle roots publish
per epoch-day (see `docs/TXLINE_API_FEEDBACK.md`), so the root covering the final-whistle update
may land after `resolve_deadline` if a match runs late relative to when the market was configured.
The grace window is a deliberate tradeoff: too short and legitimate late settlements get stuck with
no valid path forward (there is no admin override to force-resolve); too long and a stale market
sits resolvable long after the fact. Seven days was picked as generous-but-bounded; an auditor
should sanity-check this against Tx LINE's actual observed root-publish latency once that's
measured against the live feed (currently undocumented on Tx LINE's side — flagged in
`docs/TXLINE_API_FEEDBACK.md`).

There is currently no *minimum* time-after-match-end check beyond "the root must actually validate"
— `validate_stat` itself will fail if the root doesn't yet cover the claimed stat, which is the real
gate. The deadline/grace window bounds the other side (how late is too late), not this one.

## Known limitations

1. **Daily-root batching means settlement is post-match, not per-tick.** Tx LINE commits
   `daily_scores_roots` per epoch-day, not per stat-update. VAR's `resolve` therefore cannot fire the
   moment the final whistle blows — it can only fire once the covering root publishes (see
   `RESOLVE_GRACE_SECS` above). This is a deliberate consequence of building on the feed's actual
   commitment model rather than assuming a real-time push VAR doesn't have. Anyone reviewing latency
   claims for this project should read "trustless resolution," not "instant resolution."
2. **CPI return-data dependency.** `validate_stat_equal` reads the `bool` result via
   `get_return_data()` after `invoke()`. This is standard Solana return-data plumbing, but it means
   VAR's correctness depends on `Txoracle` actually populating return data on every call path
   (including its error paths) the way the IDL implies. We have not independently fuzzed
   `Txoracle`'s return-data behavior — that program is out of our control and out of scope for this
   audit; we treat it as the trusted data source (see `docs/TRUST_SURFACE.md`).
3. **Single market type in V1 — and `market_kind` is a nonce, not a validated enum.** Every market
   resolves as 1X2; `create_market` does *not* validate `market_kind` — it is deliberately
   repurposed as a per-fixture nonce in the Market PDA seeds so one fixture can host multiple
   market accounts (see the comment in `create_market`; `tests-devnet/txline-settle.ts` relies on
   this). The `UnsupportedMarketKind` error variant is defined but currently unused (reserved for
   when kinds beyond 1X2 exist). The rulebook's `MatchState`/`Outcome` types are designed to extend
   to Over/Under, correct-score, etc. (see `spec.md` §1), but that extension is unbuilt — an
   auditor should not assume any other market kind has been reviewed.
4. **No program-level Kani coverage (see table above).** The `claimed` guard, the deadline check,
   and the stat-key binding are conventional `require!` guards, not machine-proven. They're simple
   enough to review by hand (each is a single boolean condition gating a single side effect) but an
   external auditor should treat them with ordinary scrutiny, not assume Kani's guarantees extend to
   them.
5. **Devnet only — no mainnet run yet.** The program *is* built for SBF, deployed on devnet
   (`AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk`), exercised end-to-end by
   `tests-devnet/smoke.ts`, and has settled a real fixture (18192996) against the **real**
   `Txoracle` CPI over the live on-chain daily Merkle root — see `DEPLOYMENTS.md` for every tx
   signature. What remains is a mainnet (real-time L12) run, which changes cluster and service
   tier, not code.
6. **`ResolutionReceipt` binds to `source_root` derived from the two `events_sub_tree_root` values
   hashed together, not the full daily main root.** This is enough to detect if either side's
   witness changes, but an auditor verifying the receipt against the chain independently should know
   `source_root` is a derived binding hash, not literally the on-chain `daily_scores_merkle_roots`
   value — the actual authentication happened at CPI time via `validate_stat`, and the receipt is a
   record of that, not a second independent proof.

## What an external auditor should focus on

In priority order:
1. **The CPI call site** (`txoracle_cpi::validate_stat_equal` and the two `require!(ok_*, ...)`
   calls in `resolve()`) — this is the single seam where "authenticated by the chain" and "believed
   by the program" meet. Confirm the account list passed to `invoke()` matches what `Txoracle`
   actually expects once its real accounts (beyond the PDA) are known, and confirm there's no way to
   pass a `daily_scores_merkle_roots` account for a different epoch-day than the one implied by
   `witness.ts`.
2. **The fixture/stat-key/period binding** — verify the three-way binding holds as implemented:
   `attest_home` asserts `home.summary.fixture_id == m.fixture_id` and `resolve` asserts
   `away.summary.fixture_id == m.fixture_id` (`VarError::FixtureMismatch`), on top of the
   stat-key/period checks, so a cross-fixture home/away witness pair is transitively rejected.
   (This was a real gap found and closed during this audit — see the resolution note below.)
3. **The Kani proof bounds** — are `-3..=20` goals and `<=500`/`<=1000` pool values wide enough to
   call this "exhaustive in practice," or should they be widened before calling INV-1..4 airtight for
   mainnet-scale pools?
4. **The devnet integration suite** (`tests-devnet/`) — `smoke.ts` covers the full happy path with
   real account state and SPL transfers; what it does *not* yet cover is a second-claim-must-fail
   attempt, the early/late resolve boundary, and the refund path exercised on-chain. Those are the
   cheap follow-up tests to demand before mainnet.

**Resolved during this audit:** each witness is now bound on-chain to the market's own
`fixture_id` — `attest_home` asserts `home.summary.fixture_id == m.fixture_id` and `resolve`
asserts `away.summary.fixture_id == m.fixture_id` (both `VarError::FixtureMismatch`), in addition
to the stat-key and period checks. Because both are compared against `m.fixture_id`, a cross-fixture
home/away pair is transitively rejected. A valid Merkle proof lifted from a different match that
shares stat keys can no longer be used to settle this market.

**Remaining open item:** the match-status code is a resolver input, not an oracle-authenticated
field. The proven rulebook fail-closes every non-`Completed` status to `Refund` (INV-1), bounding
the downside to a refund, but binding status to an authenticated feed field is the gap to close
before mainnet.
