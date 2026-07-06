# Demo video script — 3:00

Shot list for the Superteam World Cup Hackathon submission video. Timings are targets, not
straitjackets — cut for pace, but hit every beat listed. Screen recording + voiceover; no editing
tricks that misrepresent what's actually running (see `README.md` Current Status for what's real
today versus what's still staged — the video should reflect that honestly, same as the docs).

Not yet recorded. This is the shooting plan.

---

## 0:00–0:25 — Hook: the dispute problem

**Visual:** Cold open on a headline/screenshot-style card: the 2026 Polymarket/UMA dispute — a
$60M+ market resolved against a filed 8-K, by token vote.

**Voiceover:**
"Earlier this year a $60 million prediction market got resolved the wrong way — against a filed
SEC document — because the people holding the most dispute tokens voted that way. That's not an
edge case. That's the resolution model working exactly as designed, and the design is broken.
The other failure mode is just as common: a closed oracle pipe where you're told to trust that the
right number got pushed on-chain, with no way to check it yourself.

This is VAR — Verifiable Automated Resolution. It settles World Cup markets with no token vote, no
dispute bond, no arbiter. Just a proof, that anyone can re-run."

**On screen:** Title card — "VAR: Verifiable Automated Resolution" / "Superteam World Cup Hackathon
— Track 1."

---

## 0:25–0:55 — Create + deposit

**Visual:** Terminal or CLI session. Show `create_market` being called for a real fixture (use a
2026 group-stage fixture ID, or a scenario from `scenarios/` for narrative continuity — e.g.
Argentina vs Mexico). Then two `deposit` calls into different outcome buckets (Home / Draw / Away)
from two different wallets, USDC amounts visible.

**Voiceover:**
"A market is just three USDC pools — Home, Draw, Away — tied to one fixture. Anyone can create one,
anyone can deposit. No order book, no market maker. Stake USDC on the outcome you think is right,
and wait for the match."

**On screen callouts:** `Market` PDA address, pool totals updating live (`pool_home`, `pool_draw`,
`pool_away`), the `Position` PDA being created for each depositor.

---

## 0:55–1:50 — The resolve-with-real-proof moment (centerpiece #1)

**Visual:** The match has "ended" (use a real historical scoreline for the demo fixture). Call
`resolve()` on-screen. Walk through what's actually happening, ideally with a side panel or overlay
showing the CPI:

1. The claimed scoreline (`home_goals`, `away_goals`) going in.
2. The CPI to `Txoracle::validate_stat` firing — twice, once per side — with
   `comparison = EqualTo, threshold = claimed goals`.
3. Both calls returning `true` against the on-chain `daily_scores_merkle_roots` PDA.
4. The Kani-proven `rulebook::resolve()` running on the now-authenticated `MatchState` and pool
   totals, producing an `Outcome` and a `Settlement`.
5. The `ResolutionReceipt` getting written on-chain — `source_root`, `ruleset_hash`, scoreline,
   outcome, payout split.

**Voiceover:**
"Here's the moment that matters. `resolve` doesn't take anyone's word for the score. It calls into
Tx LINE's on-chain oracle, twice, once per side, and asks it to prove the exact scoreline against
its Merkle-committed daily root. Only if both come back true does the program even look at the
result. Then — and only then — a formally verified rulebook, proven with Kani, maps the fact
pattern to an outcome and a payout split. That receipt gets written on-chain. Nobody signed off on
it. Nobody voted on it. It's just proof."

**On screen:** Highlight the two `validate_stat` CPI calls and the `true`/`true` return values, then
the `ResolutionReceipt` fields populating.

---

## 1:50–2:25 — The permissionless reverify "green check" climax (centerpiece #2)

**Visual:** A *different* wallet — one that had nothing to do with creating the market, depositing,
or resolving it — calls `reverify()`. Show the call, and the boolean result coming back `true`, big
and unmissable on screen (green checkmark overlay).

**Voiceover:**
"Now watch this. Anyone — this wallet has never touched this market before — can call `reverify`.
It re-runs the exact same rulebook function, from scratch, against the receipt that's already on
chain. Not a re-check of a signature. A full re-derivation of the outcome and the payout math. If
even one bit doesn't match, this comes back false. It doesn't."

**On screen:** The `reverify()` return value: `true`, rendered as a large green check. Optionally
show the Solana Explorer transaction link so a viewer can click through and verify it themselves
after the video.

---

## 2:25–2:45 — Edge case: penalties → Draw, and abandonment → refund

**Visual:** Quick cut to two scenario vectors from `scenarios/` running through `cargo test -p
rulebook`, or shown as a side-by-side table:
- `06_penalties-draw-argentina-france-final.json` — 2022 WC Final, 2-2 in regulation, decided 4-2 on
  penalties. Market settles **Draw** — it's a 1X2 match-result market, priced on the 90' scoreline,
  same convention as a sportsbook match-result line.
- `08_abandoned-match-refund.json` — a match abandoned mid-second-half. No valid regulation result
  exists, so the rulebook fails closed to **Refund** — every depositor gets their exact stake back,
  no fee taken.

**Voiceover:**
"Two edge cases that matter more than the happy path. A penalty shootout doesn't change the
match-result bet — it settles Draw, same as any sportsbook 1X2 line, because the market prices the
90 minutes. And if a match gets abandoned, there's no result to settle on, so the rulebook doesn't
guess — it refunds everyone, exactly their stake, no fee. Every degenerate case in this system fails
closed, never confidently wrong."

**On screen:** Show both scenario JSON files and their `expected_outcome` fields matching the actual
`cargo test` output.

---

## 2:45–3:00 — The `cargo kani` PASS shot

**Visual:** Terminal, `cargo kani -p rulebook` running to completion, all five proof harnesses
(`inv1_resolve_outcome_total_and_correct`, `inv2_settlement_conserves`,
`inv2b_two_winner_split_within_net`, `inv3_settle_fail_closed`, `inv4_resolve_deterministic`)
showing `SUCCESSFUL`/`VERIFICATION:- SUCCESSFUL`.

**Voiceover:**
"And the whole thing underneath is formally proven, not just tested. Five Kani harnesses,
exhaustively checked, not sampled: totality, conservation, fail-closed behavior, determinism. This
isn't 'we wrote tests and they passed.' It's 'we proved the rulebook cannot mint money, cannot
panic, and cannot produce two different answers for the same match.' Clone the repo. Run it
yourself. VAR — Verifiable Automated Resolution."

**On screen:** End card — repo URL, "cargo test -p rulebook && cargo kani -p rulebook," Track 1
submission mark.

---

## Descope floor backstop (only if live mainnet isn't ready by shoot day)

If the live Tx LINE mainnet activation or a real settled mainnet market isn't ready when this gets
recorded, shoot the **devnet L1 (60s-delayed) path plus an explicitly-labeled simulated replay** of
a real match's score sequence instead (per `spec.md` §1 descope floor). Say so on screen — a caption
reading "devnet + simulated feed replay, see spec.md for the mainnet activation plan" — rather than
implying a live mainnet run that didn't happen. The non-negotiable floor either way: the Kani-proven
rulebook, the on-chain resolve/claim/reverify path, and one real settled market, shown end to end.
