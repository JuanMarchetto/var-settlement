# Submission checklist — Superteam World Cup Hackathon, Track 1

Target: Superteam Earn submission before **2026-07-19**. Status as of this writing, updated as gates
close. "Done" means verified in this repo right now, not "should be fine" — see `README.md` Current
Status for the same claims in narrative form.

## Foundation

- [x] **Fresh in-window repo.** Single commit `c59db91` dated 2026-07-06, well inside the
      submission window. No history predating the hackathon.
- [x] **Spec written.** `spec.md` — thesis, scope, Tx LINE interface (verified against the IDL),
      architecture, rulebook state machine, Kani invariant list, threat model, deliverables.
- [x] **Kani-proven core.** `crates/rulebook/src/lib.rs` — **4 proof harnesses PASS** (INV-1
      totality/fail-closed, INV-2 fee conservation, INV-3 settle fail-closed, INV-4 determinism);
      transcript `docs/KANI_PROOF_TRANSCRIPT.txt`. The winner-payout solvency bound (u128 symbolic
      division, intractable for CBMC) is covered by `tests/payout_props.rs` (12k proptest cases).
- [x] **Program compiles.** `cargo check -p var-settlement` — clean, exit 0, on host.
- [x] **Rulebook test suite green.** `cargo test -p rulebook` — 25 unit tests + golden-scenario test
      (12 real-World-Cup vectors) + 3 proptest properties (12k cases). All green.
- [x] **TS SDK scaffolded.** `packages/sdk/src/txline.ts` — activation flow, PDA derivation
      (`dailyScoresRootsPda`), `StatWitness` assembly (`statWitness()`) implemented against the
      verified Tx LINE interface.

## Done — on-chain

- [x] **SBF build.** `cargo build-sbf` produces `target/deploy/var_settlement.so` (352 KB, deployable).
- [x] **Devnet deploy.** `var_settlement` live at `AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk`;
      mock `Txoracle` at `85KwDRzyZeG8wAXVCZo2CKTVor3qVcyhq7vk2yAzBJMw`. See `DEPLOYMENTS.md`.
- [x] **End-to-end integration test on devnet.** `tests-devnet/smoke.ts` drives create → deposit(Home,
      Away) → resolve(Home 2-0, CPI `validate_stat`) → reverify(`true`) → claim, with real SPL-token
      transfers. Final balances 158/40/2 (2% fee) match the rulebook exactly. **PASSED, exit 0.**
      (`litesvm` in-process tests were blocked offline by an `openssl-sys` build dep, so the check is
      done directly on devnet instead — stronger evidence anyway.)

## Done — LIVE Tx LINE

- [x] **Live Tx LINE activation.** `tests-devnet/txline-activate.ts` — real `subscribe(1,4)` on the
      devnet `Txoracle`, guest JWT, wallet-signed message, `/api/token/activate` -> live API token.
- [x] **Settled one real market against the live feed.** `tests-devnet/txline-settle.ts` — fixture
      18192996 (home 2 - 3 away), authenticated via live `stat-validation` Merkle proofs, resolved by
      two-step CPI into the **real** `Txoracle::validate_stat` over the on-chain daily root; receipt
      outcome **Away**, `reverify() -> true`, winner paid. See `DEPLOYMENTS.md`. (Not simulated.)

## Remaining gates before submission

- [ ] **Mainnet run** (real-time L12) and a `Completed`-status finished fixture for a production market.
- [ ] **Record demo video.** `docs/DEMO_VIDEO_SCRIPT.md` is the shot list (hook → create/deposit →
      resolve-with-proof → reverify green check → penalties/abandonment edge cases → `cargo kani`
      PASS). Not recorded yet.
- [ ] **README polish pass.** `README.md` is written and accurate as of this commit; revisit once
      the devnet/mainnet run and video exist so status claims can move from "staged" to "done" and
      the video gets embedded/linked.
- [ ] **Public GitHub push.** Repo has no remote configured yet — currently local-only.
- [ ] **Earn profile + KYC.** Superteam Earn submitter profile set up, KYC prepared (flagged in
      `spec.md` §8 for an Argentina payout) ahead of the deadline, not day-of.
- [ ] **Single submission before 2026-07-19.** One clean submission, not a last-minute scramble —
      everything above closed with time to spare for the actual submit step.

## Non-negotiable floor (per `spec.md` descope plan)

If time runs short before 2026-07-19, the floor that must hold regardless of what else slips:
1. The Kani-proven rulebook (already true today).
2. The on-chain resolve/claim/reverify path, exercised against real accounts (needs the SBF
   build + deploy + `litesvm`/devnet gates above).
3. One real settled market, end to end, even if that means devnet L1 (60s-delayed) plus an
   explicitly-labeled simulated replay of a real match's score sequence instead of a live mainnet
   run (`spec.md` §1, descope floor).

That combination alone clears "working, verifiable settlement" — everything else on this list
strengthens the submission but isn't the bar for a legitimate one.
