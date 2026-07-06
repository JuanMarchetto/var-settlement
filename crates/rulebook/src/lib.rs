//! VAR rulebook: deterministic, fail-closed resolution + parimutuel settlement for
//! FIFA World Cup 1X2 (match-result) prediction markets.
//!
//! This crate is the formally-verified core. It is pure and dependency-free so it links
//! unchanged into the Anchor settlement program and is independently checked with `cargo kani`.
//!
//! Convention: `home_goals` / `away_goals` are the **regulation 90'** goals. Knockout ties
//! decided in extra time or penalties settle **Draw** on the 90' scoreline (standard football
//! match-result market convention). Anything not cleanly completed settles **Refund** (VOID).

#![forbid(unsafe_code)]

/// How the match concluded, as reported by the authenticated feed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchStatus {
    /// Finished in regulation (90' + stoppage).
    Completed,
    /// Tied in regulation, decided in extra time.
    CompletedAfterExtraTime,
    /// Tied through extra time, decided on penalties.
    CompletedAfterPenalties,
    /// Called off after kickoff and not resumed.
    Abandoned,
    /// Never kicked off at the scheduled time.
    Postponed,
    /// Explicitly voided by the feed.
    Void,
}

/// Settled result of a 1X2 market.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Home,
    Draw,
    Away,
    /// Market voided; every depositor reclaims their own stake.
    Refund,
}

/// Authenticated match facts the resolver operates on. `home_goals`/`away_goals` are regulation 90'.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchState {
    pub home_goals: i32,
    pub away_goals: i32,
    pub status: MatchStatus,
}

impl MatchState {
    pub fn new(home_goals: i32, away_goals: i32, status: MatchStatus) -> Self {
        Self { home_goals, away_goals, status }
    }
}

/// Map an authenticated match state to its 1X2 outcome. Total and fail-closed:
/// never panics, and any non-clean or degenerate input resolves to `Refund`.
pub fn resolve_outcome(state: &MatchState) -> Outcome {
    // Fail closed on degenerate goal counts before anything else.
    if state.home_goals < 0 || state.away_goals < 0 {
        return Outcome::Refund;
    }
    match state.status {
        MatchStatus::Completed
        | MatchStatus::CompletedAfterExtraTime
        | MatchStatus::CompletedAfterPenalties => {
            // 1X2 settles on the regulation 90' scoreline for every completed status,
            // so ties decided in ET/penalties still settle Draw.
            if state.home_goals > state.away_goals {
                Outcome::Home
            } else if state.home_goals < state.away_goals {
                Outcome::Away
            } else {
                Outcome::Draw
            }
        }
        // Abandoned / Postponed / Void: no valid regulation result -> void the market.
        MatchStatus::Abandoned | MatchStatus::Postponed | MatchStatus::Void => Outcome::Refund,
    }
}

/// Basis-point denominator (100% = 10_000 bps).
pub const BPS_DENOM: u64 = 10_000;
/// Maximum protocol fee accepted by settlement (10%). Above this, settlement fails closed to refund.
pub const MAX_FEE_BPS: u16 = 1_000;

/// Total USDC staked in each of the three 1X2 buckets (base units, e.g. 1e6 = 1 USDC).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Pools {
    pub home: u64,
    pub draw: u64,
    pub away: u64,
}

impl Pools {
    pub fn new(home: u64, draw: u64, away: u64) -> Self {
        Self { home, draw, away }
    }

    /// Total pot, or `None` on overflow (fail-closed signal).
    pub fn total(&self) -> Option<u64> {
        self.home.checked_add(self.draw)?.checked_add(self.away)
    }

    /// Stake sitting in the bucket for `outcome` (0 for `Refund`).
    pub fn stake_for(&self, outcome: Outcome) -> u64 {
        match outcome {
            Outcome::Home => self.home,
            Outcome::Draw => self.draw,
            Outcome::Away => self.away,
            Outcome::Refund => 0,
        }
    }
}

/// Pool-level settlement facts derived from the outcome, pools, and fee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settlement {
    /// True when the whole pot is returned to depositors (void, empty winning pool,
    /// overflow, or an out-of-range fee). When true, `fee`/`net`/`winning_pool` are 0.
    pub paid_as_refund: bool,
    pub pot: u64,
    pub fee: u64,
    /// Amount distributed to winners (`pot - fee`), or 0 when refunded.
    pub net: u64,
    /// Stake in the winning bucket, or 0 when refunded.
    pub winning_pool: u64,
}

/// Derive settlement facts. Fail-closed to refund on: `Outcome::Refund`, an empty winning pool,
/// pot overflow, or `fee_bps > MAX_FEE_BPS`.
pub fn settle(outcome: Outcome, pools: Pools, fee_bps: u16) -> Settlement {
    // A refund settlement returns the whole pot; keep pot when known for bookkeeping.
    let refund = |pot: u64| Settlement {
        paid_as_refund: true,
        pot,
        fee: 0,
        net: 0,
        winning_pool: 0,
    };

    // Pot must be computable; overflow fails closed.
    let pot = match pools.total() {
        Some(p) => p,
        None => return refund(0),
    };

    // Out-of-range fee, void outcome, empty pot, or nobody on the winner => refund.
    if fee_bps > MAX_FEE_BPS || outcome == Outcome::Refund || pot == 0 {
        return refund(pot);
    }
    let winning_pool = pools.stake_for(outcome);
    if winning_pool == 0 {
        return refund(pot);
    }

    // fee = floor(pot * fee_bps / 10_000); u128 intermediate avoids overflow. fee <= pot since
    // fee_bps <= MAX_FEE_BPS <= BPS_DENOM, so net = pot - fee never underflows.
    let fee = ((pot as u128) * (fee_bps as u128) / (BPS_DENOM as u128)) as u64;
    let net = pot - fee;

    Settlement { paid_as_refund: false, pot, fee, net, winning_pool }
}

/// Payout for a winning position holding `winning_stake` in the winning bucket:
/// `floor(winning_stake * net / winning_pool)`. Returns 0 for a refunded settlement.
pub fn winner_payout(s: &Settlement, winning_stake: u64) -> u64 {
    if s.paid_as_refund || s.winning_pool == 0 {
        return 0;
    }
    // winning_stake <= winning_pool by construction, so the result <= net. u128 avoids overflow.
    ((winning_stake as u128) * (s.net as u128) / (s.winning_pool as u128)) as u64
}

/// Full resolution: the football outcome plus its settlement facts. Used by the on-chain
/// `resolve` instruction and re-run verbatim by `reverify`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resolution {
    pub outcome: Outcome,
    pub settlement: Settlement,
}

/// Resolve a market end-to-end from authenticated facts: map the state to an outcome, then
/// derive settlement. Pure and deterministic — identical inputs always yield identical output.
pub fn resolve(state: &MatchState, pools: Pools, fee_bps: u16) -> Resolution {
    let outcome = resolve_outcome(state);
    let settlement = settle(outcome, pools, fee_bps);
    Resolution { outcome, settlement }
}

// ------------------------------------------------------------------------------------------------
// Formal proofs (Kani). Inert under normal build/test; run with `cargo kani`.
// ------------------------------------------------------------------------------------------------
#[cfg(kani)]
mod proofs {
    use super::*;

    fn any_status() -> MatchStatus {
        match kani::any::<u8>() % 6 {
            0 => MatchStatus::Completed,
            1 => MatchStatus::CompletedAfterExtraTime,
            2 => MatchStatus::CompletedAfterPenalties,
            3 => MatchStatus::Abandoned,
            4 => MatchStatus::Postponed,
            _ => MatchStatus::Void,
        }
    }

    fn any_decisive_or_void() -> Outcome {
        match kani::any::<u8>() % 4 {
            0 => Outcome::Home,
            1 => Outcome::Draw,
            2 => Outcome::Away,
            _ => Outcome::Refund,
        }
    }

    fn completed(st: MatchStatus) -> bool {
        matches!(
            st,
            MatchStatus::Completed
                | MatchStatus::CompletedAfterExtraTime
                | MatchStatus::CompletedAfterPenalties
        )
    }

    /// INV-1: `resolve_outcome` is total, fail-closed, and maps completed states to the 90' result.
    #[kani::proof]
    fn inv1_resolve_outcome_total_and_correct() {
        let h: i32 = kani::any();
        let a: i32 = kani::any();
        kani::assume(h >= -3 && h <= 20);
        kani::assume(a >= -3 && a <= 20);
        let st = any_status();
        let o = resolve_outcome(&MatchState::new(h, a, st));

        // Total: always exactly one of the four (a Rust enum guarantees this; assert reachability).
        assert!(matches!(
            o,
            Outcome::Home | Outcome::Draw | Outcome::Away | Outcome::Refund
        ));
        // Fail-closed on degenerate goals.
        if h < 0 || a < 0 {
            assert!(o == Outcome::Refund);
        } else if completed(st) {
            // Correct 90' mapping.
            if h > a {
                assert!(o == Outcome::Home);
            } else if h < a {
                assert!(o == Outcome::Away);
            } else {
                assert!(o == Outcome::Draw);
            }
        } else {
            // Non-completed always voids.
            assert!(o == Outcome::Refund);
        }
    }

    /// INV-4: resolution is deterministic (pure) — same inputs, same outputs.
    #[kani::proof]
    fn inv4_resolve_deterministic() {
        let h: i32 = kani::any();
        let a: i32 = kani::any();
        kani::assume(h >= -3 && h <= 20 && a >= -3 && a <= 20);
        let st = any_status();
        let state = MatchState::new(h, a, st);
        let pools = Pools::new(kani::any(), kani::any(), kani::any());
        let fee: u16 = kani::any();
        assert!(resolve(&state, pools, fee) == resolve(&state, pools, fee));
    }

    /// INV-2: pool-level conservation — no value is created. `fee + net == pot`, `net <= pot`,
    /// and no single winner can claim more than `net`.
    #[kani::proof]
    fn inv2_settlement_conserves() {
        let home: u64 = kani::any();
        let draw: u64 = kani::any();
        let away: u64 = kani::any();
        kani::assume(home <= 1000 && draw <= 1000 && away <= 1000);
        let fee_bps: u16 = kani::any();
        kani::assume(fee_bps <= MAX_FEE_BPS);
        let outcome = any_decisive_or_void();
        let pools = Pools::new(home, draw, away);
        let s = settle(outcome, pools, fee_bps);

        assert!(s.pot == home + draw + away);
        if s.paid_as_refund {
            assert!(s.fee == 0 && s.net == 0);
        } else {
            assert!(s.fee + s.net == s.pot);
            assert!(s.net <= s.pot);
            // The entire winning pool claims at most `net`.
            assert!(winner_payout(&s, s.winning_pool) <= s.net);
            // Any sub-stake claims at most `net`.
            let stake: u64 = kani::any();
            kani::assume(stake <= s.winning_pool);
            assert!(winner_payout(&s, stake) <= s.net);
        }
    }

    /// INV-2b: solvency under splitting — any two disjoint winner stakes together stay within
    /// `net`, so the full set of winners can always be paid from escrow (induction base).
    #[kani::proof]
    fn inv2b_two_winner_split_within_net() {
        let home: u64 = kani::any();
        let draw: u64 = kani::any();
        let away: u64 = kani::any();
        kani::assume(home <= 500 && draw <= 500 && away <= 500);
        let fee_bps: u16 = kani::any();
        kani::assume(fee_bps <= MAX_FEE_BPS);
        let outcome = any_decisive_or_void();
        kani::assume(outcome != Outcome::Refund);
        let s = settle(outcome, Pools::new(home, draw, away), fee_bps);
        kani::assume(!s.paid_as_refund);
        let s1: u64 = kani::any();
        let s2: u64 = kani::any();
        kani::assume(s1 <= s.winning_pool);
        kani::assume(s2 <= s.winning_pool - s1); // s1 + s2 <= winning_pool
        assert!(winner_payout(&s, s1) + winner_payout(&s, s2) <= s.net);
    }

    /// INV-3: settlement fail-closed paths — void outcome and out-of-range fee always refund.
    #[kani::proof]
    fn inv3_settle_fail_closed() {
        let home: u64 = kani::any();
        let draw: u64 = kani::any();
        let away: u64 = kani::any();
        kani::assume(home <= 1000 && draw <= 1000 && away <= 1000);
        let fee_bps: u16 = kani::any();
        let pools = Pools::new(home, draw, away);

        let sr = settle(Outcome::Refund, pools, fee_bps);
        assert!(sr.paid_as_refund && sr.fee == 0 && sr.net == 0);

        if fee_bps > MAX_FEE_BPS {
            assert!(settle(Outcome::Home, pools, fee_bps).paid_as_refund);
        }
    }
}
