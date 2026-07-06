//! Randomized property tests for the settlement arithmetic whose Kani proof is intractable
//! (u128 division by a symbolic divisor). proptest exercises thousands of cases per property,
//! at full USDC magnitude, covering what CBMC cannot bit-blast.

use proptest::prelude::*;
use rulebook::{settle, winner_payout, Outcome, Pools};

fn outcome(i: u8) -> Outcome {
    [Outcome::Home, Outcome::Draw, Outcome::Away][(i % 3) as usize]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 4000, ..ProptestConfig::default() })]

    /// A single winner never claims more than `net`, at full USDC magnitude.
    #[test]
    fn winner_payout_never_exceeds_net(
        home in 0u64..1_000_000_000_000,
        draw in 0u64..1_000_000_000_000,
        away in 0u64..1_000_000_000_000,
        fee_bps in 0u16..=1000,
        oi in 0u8..3,
        frac in 0u64..=10_000,
    ) {
        let s = settle(outcome(oi), Pools::new(home, draw, away), fee_bps);
        prop_assume!(!s.paid_as_refund);
        let stake = if s.winning_pool == 0 { 0 } else { (s.winning_pool / 10_000) * frac.min(10_000) };
        prop_assume!(stake <= s.winning_pool);
        prop_assert!(winner_payout(&s, stake) <= s.net);
    }

    /// Solvency: any two disjoint winner stakes together stay within `net`
    /// (the induction base for the whole winner set being payable from escrow).
    #[test]
    fn two_winners_split_within_net(
        home in 1u64..1_000_000_000_000,
        draw in 0u64..1_000_000_000_000,
        away in 0u64..1_000_000_000_000,
        fee_bps in 0u16..=1000,
        f1 in 0u64..=5_000,
        f2 in 0u64..=5_000,
    ) {
        let s = settle(Outcome::Home, Pools::new(home, draw, away), fee_bps);
        prop_assume!(!s.paid_as_refund && s.winning_pool > 0);
        let s1 = (s.winning_pool / 10_000) * f1;
        let s2 = (s.winning_pool / 10_000) * f2;
        prop_assume!(s1 + s2 <= s.winning_pool);
        prop_assert!(winner_payout(&s, s1) + winner_payout(&s, s2) <= s.net);
    }

    /// The full winning pool claims exactly `net` minus flooring dust (never more, never negative).
    #[test]
    fn full_pool_claims_at_most_net(
        home in 1u64..1_000_000_000_000,
        draw in 0u64..1_000_000_000_000,
        away in 0u64..1_000_000_000_000,
        fee_bps in 0u16..=1000,
    ) {
        let s = settle(Outcome::Home, Pools::new(home, draw, away), fee_bps);
        prop_assume!(!s.paid_as_refund);
        prop_assert!(winner_payout(&s, s.winning_pool) <= s.net);
    }
}
