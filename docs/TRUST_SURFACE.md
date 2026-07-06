# Trust surface

What VAR asks you to trust, what it doesn't, and what happens when something goes wrong. Written
against `programs/var-settlement/src/lib.rs` and `crates/rulebook/src/lib.rs` as they exist today.

## The honest claim

VAR does not claim to know the truth about a football match independent of any data source — that
would be a different, much harder product (and not what a settlement engine is for). The claim is
narrower and, we think, honest:

> **Given Tx LINE's signed on-chain feed, settlement is correct, deterministic, and independently
> re-checkable. No human, token holder, or arbiter can override it, and anyone can re-run the proof
> from scratch and get the same answer.**

That's strictly more than "trust the oracle and take the receipt on faith." It's also strictly less
than "VAR knows who really won" — it can't, and doesn't pretend to. If Tx LINE's feed is wrong, VAR
settles wrong, in the same way a scoreboard settles wrong if someone changes the actual scoreline
before it's recorded. The engineering problem VAR solves is: assuming the feed is right, remove
every other point of failure between "the match ended" and "the money moved."

## Trusted

**TxODDS as the source of truth for match facts.** This is unavoidable — Tx LINE is the hackathon's
data sponsor and the only source of an on-chain, Merkle-committed football feed VAR integrates
against. VAR does not second-guess `daily_scores_roots`; it authenticates against it.

That's the entire trusted set. Everything downstream of the feed — the mapping from facts to money —
is proven, not trusted.

## Untrusted (and why it can't hurt you)

**The `resolve` caller.** `resolve` in `programs/var-settlement/src/lib.rs` is permissionless —
`Resolve` accounts require only `resolver: Signer<'info>`, no allowlist, no admin key. Anyone can
call it. That's fine because a malicious or careless caller cannot:

- **Lie about the score.** `resolve` doesn't take the caller's word for `home_goals`/`away_goals`.
  It CPIs `Txoracle::validate_stat` once per side with `comparison = EqualTo, threshold = <claimed
  value>` (`txoracle_cpi::validate_stat_equal`). Both calls must return `true` against the on-chain
  `daily_scores_merkle_roots` PDA, or `resolve` errors with `StatNotAuthenticated`. A caller who
  submits a false scoreline just gets a failed transaction, not a bad settlement.
- **Point the proof at the wrong stat.** Each `StatWitness` is bound to the market's configured
  `home_stat_key`/`away_stat_key`/`period` before it's trusted (`StatKeyMismatch` /
  `StatPeriodMismatch` checks). Without this, a caller could authenticate *some* true stat from the
  feed (e.g. a different fixture, or a corner-kick count) and pass it off as the goals stat.
- **Resolve early.** `require!(now <= m.resolve_deadline.saturating_add(RESOLVE_GRACE_SECS), ...)` —
  resolution is only accepted inside the configured window (deadline plus a 7-day grace period for
  root-publish lag). There's no path to settling a market before its match has actually concluded
  and the covering root is live.
- **Resolve twice or flip the result.** `require!(m.status == MarketStatus::Open as u8,
  VarError::AlreadySettled)` at the top of `resolve` — once `Market.status` flips to `Settled` the
  instruction is a no-op path (it errors out), permanently. There is no `un-resolve`, no admin
  override, no re-resolve-with-different-data instruction anywhere in the program.
- **Double-pay a position.** `claim()` requires `!p.claimed` and sets `p.claimed = true` before any
  transfer executes. A `Position` PDA pays out at most once, structurally, regardless of how many
  times `claim` is invoked against it.
- **Mint value.** Every payout is `winner_payout()`, which is bounded by Kani's INV-2/INV-2b to
  never exceed `net` (pot minus fee) in aggregate. The vault can't be drained beyond what was
  escrowed.

**The depositor.** Depositors can only stake into one of the three defined outcome buckets
(`InvalidOutcome` guards anything outside `Home`/`Draw`/`Away`), and stakes are non-negative amounts
transferred via SPL token — there's no way to under-fund the vault relative to recorded pool totals.

**The market creator.** `create_market` is also permissionless, but a creator can't set an
unreasonable fee (`FeeTooHigh` rejects `fee_bps > MAX_FEE_BPS`, capped at 10%) or a deadline in the
past (`DeadlineInPast`). A creator picks *which* fixture/stat keys a market tracks, not what the
outcome will be.

## Fail-closed behavior — the default is "give the money back," never "guess"

Every degenerate or ambiguous path in `rulebook::resolve_outcome` / `rulebook::settle` resolves to
`Outcome::Refund`, proven by Kani's INV-1 and INV-3:

- Match status `Abandoned`, `Postponed`, or `Void` → `Refund`. No partial credit for an in-progress
  scoreline at the time of abandonment.
- Negative goal counts (a degenerate/corrupted input) → `Refund`, never a panic, never an award.
- Fee configured above `MAX_FEE_BPS` → `Refund` (should be unreachable given `create_market`'s
  check, but the settlement function fails closed anyway rather than trusting the caller).
- A decisive outcome with an empty winning pool (nobody backed the actual winner) → `Refund` instead
  of a division that would either panic or, worse, silently pay zero to a nonexistent winner set
  while keeping everyone else's stake.
- On `Refund`, every depositor reclaims exactly their own total stake across all three buckets
  (`fee == 0`) — nobody profits or loses from a voided market.

The rulebook never panics on any reachable input (Kani proves totality over the full swept range),
and there is no code path where an ambiguous fact pattern produces a confident-looking wrong answer
instead of a refund.

## What's outside this trust surface entirely

- **The Tx LINE feed's own correctness** — VAR authenticates *against* the feed, it doesn't audit
  the feed's ingestion pipeline. If TxODDS's off-chain scouts get a goal wrong before it's
  committed to the Merkle root, VAR settles on that wrong root, correctly and deterministically. This
  is the one input VAR cannot verify past, by design and by necessity — see `docs/AUDIT.md` for how
  this is scoped as a known limitation, not a hidden one.
- **Program upgrade authority** (out of scope for this doc — see `docs/AUDIT.md` for the ops-level
  discussion of deploy keys).
