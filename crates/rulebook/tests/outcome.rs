//! RED-first behavior tests for `resolve_outcome`.
//! Convention under test: 1X2 settles on the REGULATION 90' scoreline; non-clean states VOID.

use rulebook::{resolve_outcome, MatchState, MatchStatus, Outcome};

fn state(h: i32, a: i32, s: MatchStatus) -> MatchState {
    MatchState::new(h, a, s)
}

#[test]
fn home_win_in_regulation() {
    assert_eq!(resolve_outcome(&state(2, 0, MatchStatus::Completed)), Outcome::Home);
}

#[test]
fn away_win_in_regulation() {
    assert_eq!(resolve_outcome(&state(1, 3, MatchStatus::Completed)), Outcome::Away);
}

#[test]
fn regulation_draw() {
    assert_eq!(resolve_outcome(&state(1, 1, MatchStatus::Completed)), Outcome::Draw);
}

#[test]
fn goalless_draw() {
    assert_eq!(resolve_outcome(&state(0, 0, MatchStatus::Completed)), Outcome::Draw);
}

#[test]
fn knockout_decided_in_extra_time_settles_draw_on_90() {
    // Tied 1-1 at 90', a goal in ET decides the tie. Match-result market settles DRAW.
    assert_eq!(
        resolve_outcome(&state(1, 1, MatchStatus::CompletedAfterExtraTime)),
        Outcome::Draw
    );
}

#[test]
fn knockout_decided_on_penalties_settles_draw_on_90() {
    // 0-0 through extra time, decided on penalties. Match-result market settles DRAW.
    assert_eq!(
        resolve_outcome(&state(0, 0, MatchStatus::CompletedAfterPenalties)),
        Outcome::Draw
    );
}

#[test]
fn abandoned_match_voids() {
    assert_eq!(resolve_outcome(&state(1, 0, MatchStatus::Abandoned)), Outcome::Refund);
}

#[test]
fn postponed_match_voids() {
    assert_eq!(resolve_outcome(&state(0, 0, MatchStatus::Postponed)), Outcome::Refund);
}

#[test]
fn explicit_void_voids() {
    assert_eq!(resolve_outcome(&state(2, 2, MatchStatus::Void)), Outcome::Refund);
}

#[test]
fn negative_goals_fail_closed_to_refund() {
    // Degenerate/corrupt input must never mis-award; it voids.
    assert_eq!(resolve_outcome(&state(-1, 0, MatchStatus::Completed)), Outcome::Refund);
    assert_eq!(resolve_outcome(&state(0, -5, MatchStatus::Completed)), Outcome::Refund);
}
