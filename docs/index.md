# VAR — Verifiable Automated Resolution

**Technical documentation.** Trustless, formally-verified settlement for FIFA World Cup 2026 1X2
match-result markets on Solana, resolved by CPI into TxODDS **Tx LINE**'s on-chain Merkle feed.

Superteam World Cup Hackathon — Track 1: *Prediction Markets & Settlement*.

---

## Verify it yourself (no trust required)

| What | How |
|---|---|
| **A real fixture, settled on-chain** | [Market `GaiXEuSB…v8T3` on Solana Explorer](https://explorer.solana.com/address/GaiXEuSBb3spjoptxHCoyScycN4sCy164jCF3jT9v8T3?cluster=devnet) — fixture 18192996, scoreline 2–3, outcome **Away** |
| **Re-derive it from any wallet** | `cd tests-devnet && bun install && bun run reverify.ts GaiXEuSBb3spjoptxHCoyScycN4sCy164jCF3jT9v8T3` → `true` |
| **The 4 formal proofs** | `cd crates/rulebook && cargo kani` → `4 successfully verified harnesses, 0 failures` |
| **The test suite** | `cargo test -p rulebook` → 22 unit tests + 12 golden World Cup vectors + 12,000 proptest cases |
| **The deployed program** | [`AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk`](https://explorer.solana.com/address/AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk?cluster=devnet) |

---

## Documentation index

- **[Design spec](https://github.com/JuanMarchetto/var-settlement/blob/master/spec.md)** — architecture, rulebook state machine, Tx LINE interface, threat model, invariant list.
- **[Trust surface](TRUST_SURFACE.html)** — what VAR asks you to trust, what it doesn't, and the fail-closed behaviour of every degenerate path.
- **[Self-audit](AUDIT.html)** — proven invariants, arithmetic discipline, known limitations, and what an external auditor should focus on.
- **[Deployments & on-chain evidence](https://github.com/JuanMarchetto/var-settlement/blob/master/DEPLOYMENTS.md)** — every program id, market account, and transaction signature, with re-run commands.
- **[Kani proof transcript](https://github.com/JuanMarchetto/var-settlement/blob/master/docs/KANI_PROOF_TRANSCRIPT.txt)** — the committed verifier output.
- **[Tx LINE API feedback](TXLINE_API_FEEDBACK.html)** — builder notes on the sponsor's API and docs.
- **[Submission summary](https://github.com/JuanMarchetto/var-settlement/blob/master/SUBMISSION.md)** — the pitch, the evidence, the honest status.

---

## How settlement works

```
create_market(fixture_id, home_stat_key, away_stat_key, period, fee_bps, resolve_deadline)
    -> Market PDA + USDC vault, three parimutuel pools (Home/Draw/Away)

deposit(outcome, amount)
    -> USDC into escrow; Position PDA + pool totals updated

attest_home(home: StatWitness)              // permissionless, step 1 of 2
    -> binds witness to the market's fixture_id / stat_key / period
    -> CPI Txoracle::validate_stat(EqualTo, threshold = home_goals)
    -> must return true or it fails closed (StatNotAuthenticated)

resolve(away: StatWitness, status_code)     // permissionless, step 2 of 2
    -> same checks + CPI for away_goals; requires home already attested
    -> rulebook::resolve(MatchState, Pools, fee_bps) -> Outcome + Settlement
    -> writes ResolutionReceipt, flips Market.status = Settled

claim()      -> floor(stake * net / winning_pool), or full refund; paid at most once
reverify()   -> bool, permissionless read-only re-derivation of the whole resolution
```

Two transactions because both Merkle proofs together exceed Solana's 1232-byte limit.

---

## The formal-verification core

`crates/rulebook` is a pure, dependency-free Rust crate with no Solana imports. It compiles
unchanged into the Anchor program **and** is independently model-checked with
[Kani](https://model-checking.github.io/kani/) — so the guarantee is decoupled from the proof
transport. Four harnesses, all passing:

- **INV-1 totality / fail-closed** — `resolve_outcome` is total, never panics; degenerate goal
  counts and every non-`Completed` status resolve to `Refund`.
- **INV-2 conservation** — `pot == home + draw + away`; on a paying settlement `fee + net == pot`
  and `net <= pot`; on a refund `fee == 0, net == 0`. The program cannot mint value.
- **INV-3 settlement fail-closed** — `Refund` and any out-of-range `fee_bps` always settle as a
  zero-fee full refund.
- **INV-4 determinism** — identical inputs always yield an identical resolution.

The per-winner payout bound divides by a symbolic `u128` divisor (intractable for the model
checker), so it is covered by 3 property invariants × 4,000 cases in `tests/payout_props.rs`.

`resolve` and `reverify` call the *same* pure function, so a green `reverify` means the receipt is
reproducible from scratch — not merely internally consistent.

---

## Honest status

Devnet is the target and it is sufficient: Tx LINE's free World Cup access grant is the devnet L1
tier. The goal counts are Merkle-authenticated on-chain; the **match-status code is a resolver
input**, not an oracle-authenticated field — the proven rulebook fail-closes every non-`Completed`
status to `Refund`, so the worst a caller can force is a refund, never a fabricated decisive
result. Binding status to an authenticated feed field is the gap to close before mainnet. Full
detail in the [self-audit](AUDIT.html).

---

[Source on GitHub](https://github.com/JuanMarchetto/var-settlement) · MIT licensed
