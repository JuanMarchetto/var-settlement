//! Golden test vectors: real World Cup matches (and edge-case hypotheticals) in `scenarios/*.json`.
//! Validates the rulebook end-to-end against hand-checked expected outcomes.

use rulebook::{resolve, MatchState, MatchStatus, Outcome, Pools};
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Deserialize)]
struct PoolsJson {
    home: u64,
    draw: u64,
    away: u64,
}

#[derive(Deserialize)]
struct Scenario {
    name: String,
    #[allow(dead_code)]
    real_match: String,
    home_goals: i32,
    away_goals: i32,
    status: String,
    fee_bps: u16,
    pools: PoolsJson,
    expected_outcome: String,
    expected_paid_as_refund: bool,
}

fn status(s: &str) -> MatchStatus {
    match s {
        "Completed" => MatchStatus::Completed,
        "CompletedAfterExtraTime" => MatchStatus::CompletedAfterExtraTime,
        "CompletedAfterPenalties" => MatchStatus::CompletedAfterPenalties,
        "Abandoned" => MatchStatus::Abandoned,
        "Postponed" => MatchStatus::Postponed,
        "Void" => MatchStatus::Void,
        other => panic!("unknown status in scenario: {other}"),
    }
}

fn outcome(s: &str) -> Outcome {
    match s {
        "Home" => Outcome::Home,
        "Draw" => Outcome::Draw,
        "Away" => Outcome::Away,
        "Refund" => Outcome::Refund,
        other => panic!("unknown outcome in scenario: {other}"),
    }
}

#[test]
fn all_golden_scenarios_resolve_as_expected() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "json").unwrap_or(false))
        .collect();
    files.sort();
    assert!(files.len() >= 12, "expected >=12 golden scenarios, found {}", files.len());

    for path in files {
        let raw = std::fs::read_to_string(&path).unwrap();
        let sc: Scenario = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("bad JSON in {}: {e}", path.display()));

        let r = resolve(
            &MatchState::new(sc.home_goals, sc.away_goals, status(&sc.status)),
            Pools::new(sc.pools.home, sc.pools.draw, sc.pools.away),
            sc.fee_bps,
        );

        assert_eq!(
            r.outcome,
            outcome(&sc.expected_outcome),
            "outcome mismatch in '{}' ({})",
            sc.name,
            path.display()
        );
        assert_eq!(
            r.settlement.paid_as_refund,
            sc.expected_paid_as_refund,
            "paid_as_refund mismatch in '{}' ({})",
            sc.name,
            path.display()
        );

        // Conservation must hold on every real vector too.
        let s = &r.settlement;
        if s.paid_as_refund {
            assert_eq!(s.fee, 0);
            assert_eq!(s.net, 0);
        } else {
            assert_eq!(s.fee + s.net, s.pot);
            assert!(rulebook::winner_payout(s, s.winning_pool) <= s.net);
        }
    }
}
