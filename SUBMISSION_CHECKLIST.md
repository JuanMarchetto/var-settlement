# Submission checklist — Superteam World Cup Hackathon, Track 1

Target: Superteam Earn submission before **2026-07-19**. Status as of this writing, updated as gates
close. "Done" means verified in this repo right now, not "should be fine" — see `README.md` Current
Status for the same claims in narrative form.

## Foundation

- [x] **Fresh in-window repo.** Single commit `c59db91` dated 2026-07-06, well inside the
      submission window. No history predating the hackathon.
- [x] **Spec written.** `spec.md` — thesis, scope, Tx LINE interface (verified against the IDL),
      architecture, rulebook state machine, Kani invariant list, threat model, deliverables.
- [x] **Kani-proven core.** `crates/rulebook/src/lib.rs` — five proof harnesses (INV-1, INV-2,
      INV-2b, INV-3, INV-4) written and running under `cargo kani -p rulebook`.
- [x] **Program compiles.** `cargo check -p var-settlement` — clean, exit 0, on host.
- [x] **Rulebook test suite green.** `cargo test -p rulebook` — 22 unit tests
      (`crates/rulebook/tests/outcome.rs`, `resolve.rs`, `settlement.rs`) + the golden-scenario test
      (`golden.rs`) covering all 12 real-World-Cup vectors in `scenarios/*.json`.
- [x] **TS SDK scaffolded.** `packages/sdk/src/txline.ts` — activation flow, PDA derivation
      (`dailyScoresRootsPda`), `StatWitness` assembly (`statWitness()`) implemented against the
      verified Tx LINE interface.

## Remaining gates before submission

- [ ] **SBF build.** `cargo build-sbf` / `anchor build` for `programs/var-settlement`. Host compile
      is clean; the BPF/SBF target build hasn't been run. `target/deploy/` currently holds only the
      program keypair, no built `.so`.
- [ ] **`litesvm` integration tests.** Happy path (create → deposit → resolve-with-mocked-Txoracle →
      claim → reverify), double-claim guard, early/late-resolve guard, refund path. `tests/` is
      currently empty — this is unwritten, not just unrun.
- [ ] **Devnet deploy.** Deploy `programs/var-settlement` under `AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk`
      (already declared in `declare_id!`) to devnet.
- [ ] **Live Tx LINE activation.** Run the 4-step activation (`packages/sdk/src/txline.ts::activate`)
      for real against a funded wallet — the free World Cup tier is verified reachable and coded, but
      `subscribe()` is currently a stub that throws until it's wired against a live `subscribe`
      transaction.
- [ ] **Settle one real market.** Create a market for a real fixture, take deposits, resolve it
      against the live Tx LINE feed (devnet L1 or mainnet, per `spec.md`'s descope ladder), claim
      payouts, and get a `true` from `reverify()` — the actual end-to-end proof, not a simulated one.
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
