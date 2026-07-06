//! RED-first tests for the end-to-end `resolve` combinator (outcome + settlement).

use rulebook::{resolve, MatchState, MatchStatus, Outcome, Pools};

const USDC: u64 = 1_000_000;

#[test]
fn home_win_resolves_and_settles() {
    let r = resolve(
        &MatchState::new(2, 0, MatchStatus::Completed),
        Pools::new(40 * USDC, 25 * USDC, 35 * USDC),
        200,
    );
    assert_eq!(r.outcome, Outcome::Home);
    assert_eq!(r.settlement.paid_as_refund, false);
    assert_eq!(r.settlement.net, 98 * USDC);
    assert_eq!(r.settlement.winning_pool, 40 * USDC);
}

#[test]
fn abandoned_resolves_refund_and_settles_as_refund() {
    let r = resolve(
        &MatchState::new(1, 0, MatchStatus::Abandoned),
        Pools::new(10 * USDC, 10 * USDC, 10 * USDC),
        200,
    );
    assert_eq!(r.outcome, Outcome::Refund);
    assert_eq!(r.settlement.paid_as_refund, true);
    assert_eq!(r.settlement.fee, 0);
}

#[test]
fn penalties_decider_settles_draw() {
    let r = resolve(
        &MatchState::new(0, 0, MatchStatus::CompletedAfterPenalties),
        Pools::new(30 * USDC, 30 * USDC, 40 * USDC),
        0,
    );
    assert_eq!(r.outcome, Outcome::Draw);
    assert_eq!(r.settlement.paid_as_refund, false);
    assert_eq!(r.settlement.winning_pool, 30 * USDC);
}
