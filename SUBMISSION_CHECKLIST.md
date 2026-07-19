# Submission checklist — Superteam World Cup Hackathon, Track 1

Target: Superteam Earn submission before **2026-07-19**. Status as of this writing, updated as gates
close. "Done" means verified in this repo right now, not "should be fine" — see `README.md` Current
Status for the same claims in narrative form.

## Foundation

- [x] **Fresh in-window repo.** First commit `c59db91` dated 2026-07-06; every commit in the
      history is inside the hackathon submission window.
- [x] **Spec written.** `spec.md` — thesis, scope, Tx LINE interface (verified against the IDL),
      architecture, rulebook state machine, Kani invariant list, threat model, deliverables.
- [x] **Kani-proven core.** `crates/rulebook/src/lib.rs` — **4 proof harnesses PASS** (INV-1
      totality/fail-closed, INV-2 fee conservation, INV-3 settle fail-closed, INV-4 determinism);
      transcript `docs/KANI_PROOF_TRANSCRIPT.txt`. The winner-payout solvency bound (u128 symbolic
      division, intractable for CBMC) is covered by `tests/payout_props.rs` (12k proptest cases).
- [x] **Program compiles.** `cargo check -p var-settlement` — clean, exit 0, on host.
- [x] **Rulebook test suite green.** `cargo test -p rulebook` — 22 unit tests + golden-scenario test
      (12 real-World-Cup vectors) + 3 proptest properties (12k cases). All green.
- [x] **TS SDK scaffolded.** `packages/sdk/src/txline.ts` — activation flow, PDA derivation
      (`dailyScoresRootsPda`), `StatWitness` assembly (`statWitness()`) implemented against the
      verified Tx LINE interface.

## Done — on-chain

- [x] **SBF build.** `cargo build-sbf` produces `target/deploy/var_settlement.so` (355,512 bytes / 347 KiB, deployable).
- [x] **Devnet deploy.** `var_settlement` live at `AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk`;
      mock `Txoracle` at `85KwDRzyZeG8wAXVCZo2CKTVor3qVcyhq7vk2yAzBJMw`. See `DEPLOYMENTS.md`.
- [x] **End-to-end integration test on devnet.** `tests-devnet/smoke.ts` drives create → deposit(Home,
      Away) → attest_home → resolve(Home 2-0, CPI `validate_stat`) → reverify(`true`) → claim, with
      real SPL-token transfers. Final balances 158/40/2 (2% fee) match the rulebook exactly.
      **PASSED, exit 0.**
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

- [ ] **Record demo video.** `docs/DEMO_VIDEO_SCRIPT.md` is the shot list (hook → Kani PASS →
      real-feed settlement → stranger-wallet reverify green check → Explorer). Not recorded yet —
      the only open content gate.
- [x] **README polish pass.** Done 2026-07-19: status claims moved from "staged" to "done" with
      the devnet/live-feed evidence and tx links; Kani claims synced to the 4 verified harnesses;
      two-step `attest_home` + `resolve` documented.
- [x] **Public GitHub push.** Live and public at https://github.com/JuanMarchetto/var-settlement
      (topics, description, homepage, and MIT license set).
- [ ] **Earn profile ready.** Superteam Earn submitter profile set up ahead of the submit click.
- [ ] **Single submission on 2026-07-19.** One clean submission — everything above closed, then
      the actual submit step with the video link pasted into `SUBMISSION.md` and the Earn form.

## Explicitly descoped (not submission gates)

- **Mainnet run** (real-time L12) and a `Completed`-status finished fixture for a production
  market. Devnet is the target and it's sufficient — Tx LINE's free World Cup tier is devnet L1,
  and mainnet is not a hackathon requirement (see `SUBMISSION.md` §Honest status). Post-hackathon.

## Non-negotiable floor (per `spec.md` descope plan) — **all three cleared**

The floor that had to hold regardless of what else slipped:
1. ✅ The Kani-proven rulebook — 4 harnesses, 0 failures.
2. ✅ The on-chain resolve/claim/reverify path exercised against real accounts — SBF build, devnet
   deploy, and the `tests-devnet/smoke.ts` lifecycle run with real SPL transfers.
3. ✅ One real settled market, end to end — and better than the floor allowed: not a simulated
   replay but a **live** authentication of fixture 18192996's goal counts against Tx LINE's real
   on-chain daily Merkle root, re-derivable by any stranger wallet via `reverify`.

That combination clears "working, verifiable settlement" — the remaining gates above strengthen
the submission but aren't the bar for a legitimate one.
