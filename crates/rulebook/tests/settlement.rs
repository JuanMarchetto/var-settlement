//! RED-first behavior tests for parimutuel settlement (`settle` + `winner_payout`).
//! USDC base units (1_000_000 = 1 USDC).

use rulebook::{settle, winner_payout, Outcome, Pools, Settlement};

const USDC: u64 = 1_000_000;

#[test]
fn decisive_win_applies_fee_and_sets_net() {
    // pot = 100 USDC, fee 2% => fee 2 USDC, net 98 USDC. Home wins; home pool = 40 USDC.
    let pools = Pools::new(40 * USDC, 25 * USDC, 35 * USDC);
    let s = settle(Outcome::Home, pools, 200);
    assert_eq!(s.paid_as_refund, false);
    assert_eq!(s.pot, 100 * USDC);
    assert_eq!(s.fee, 2 * USDC);
    assert_eq!(s.net, 98 * USDC);
    assert_eq!(s.winning_pool, 40 * USDC);
}

#[test]
fn two_winners_split_net_pro_rata() {
    // Home wins, home pool 40 USDC split between stakes of 30 and 10; net 98 USDC.
    let pools = Pools::new(40 * USDC, 25 * USDC, 35 * USDC);
    let s = settle(Outcome::Home, pools, 200);
    let a = winner_payout(&s, 30 * USDC); // 30/40 of 98 = 73.5
    let b = winner_payout(&s, 10 * USDC); // 10/40 of 98 = 24.5
    assert_eq!(a, 73_500_000);
    assert_eq!(b, 24_500_000);
    // Conservation: distributed <= net.
    assert!(a + b <= s.net);
}

#[test]
fn draw_outcome_pays_draw_backers() {
    let pools = Pools::new(40 * USDC, 20 * USDC, 40 * USDC);
    let s = settle(Outcome::Draw, pools, 0);
    assert_eq!(s.winning_pool, 20 * USDC);
    assert_eq!(s.net, 100 * USDC); // no fee
    assert_eq!(winner_payout(&s, 20 * USDC), 100 * USDC); // sole backer takes the whole net
}

#[test]
fn refund_outcome_is_paid_as_refund_with_no_fee() {
    let pools = Pools::new(10 * USDC, 10 * USDC, 10 * USDC);
    let s = settle(Outcome::Refund, pools, 200);
    assert_eq!(s.paid_as_refund, true);
    assert_eq!(s.fee, 0);
    assert_eq!(s.net, 0);
    assert_eq!(s.winning_pool, 0);
}

#[test]
fn nobody_backed_the_winner_fails_closed_to_refund() {
    // Away wins but the away pool is empty. Fail closed: refund everyone.
    let pools = Pools::new(50 * USDC, 50 * USDC, 0);
    let s = settle(Outcome::Away, pools, 200);
    assert_eq!(s.paid_as_refund, true);
    assert_eq!(s.fee, 0);
    assert_eq!(s.net, 0);
}

#[test]
fn out_of_range_fee_fails_closed_to_refund() {
    let pools = Pools::new(10 * USDC, 10 * USDC, 10 * USDC);
    let s = settle(Outcome::Home, pools, 5_000); // 50% > MAX_FEE_BPS
    assert_eq!(s.paid_as_refund, true);
}

#[test]
fn winner_payout_is_zero_for_refunded_settlement() {
    let s = Settlement { paid_as_refund: true, pot: 30 * USDC, fee: 0, net: 0, winning_pool: 0 };
    assert_eq!(winner_payout(&s, 10 * USDC), 0);
}

#[test]
fn dust_from_flooring_never_exceeds_net() {
    // Three equal winners of an indivisible net: floor leaves dust in the vault, never over-pays.
    let pools = Pools::new(3, 0, 0); // 3 base units all on Home
    let s = settle(Outcome::Home, pools, 0);
    assert_eq!(s.net, 3);
    // net=3, winning_pool=3, each unit stake claims floor(1*3/3)=1; three winners => 3 total, 0 dust here.
    let each = winner_payout(&s, 1);
    assert_eq!(each, 1);
    assert!(each * 3 <= s.net);
}

#[test]
fn empty_pot_refunds() {
    let s = settle(Outcome::Home, Pools::new(0, 0, 0), 200);
    assert_eq!(s.paid_as_refund, true);
}
