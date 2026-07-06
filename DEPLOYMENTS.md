# Deployments & on-chain evidence

## Devnet (live)

| Program | Address | Deploy tx |
|---|---|---|
| **var_settlement** | `AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk` | `3nP3gVyarNKeyhQ66meUA6VbZL52f6WTM1fNkjv4axKXt73wiy4E3YZ6NejyCBr5SUhGgWaqmf4PMAXjBS1CQPiY` |
| **mock_txoracle** (test only) | `85KwDRzyZeG8wAXVCZo2CKTVor3qVcyhq7vk2yAzBJMw` | `4yFVJfMdkPwixdZN8G8RCnWGqqrVaJw2dJcnZvN5x6Vutdw8NkJJWRnNyHSvJLtg7qoEXjmYG4ebRAxh9HvkA1NE` |

- Upgrade authority: `BExXP4aGnZRq3PMw4vpY7KHgdeGPCfogao66CpsTMUjf`
- Explorer: https://explorer.solana.com/address/AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk?cluster=devnet

> `mock_txoracle` stands in for Tx LINE's `Txoracle::validate_stat` (always attests `true`) so the
> full flow can run on devnet without the live Merkle feed. **Never on mainnet.** A mainnet market
> passes the real Tx LINE program id as `txoracle_program`.

## End-to-end smoke test (`tests-devnet/smoke.ts`) — PASSED

Full lifecycle on devnet with real transactions and real SPL-token transfers:
`create_market -> deposit(Home 40, Away 60) -> resolve(Home 2-0, CPI validate_stat) -> reverify -> claim`.

A representative run:

| Step | Signature |
|---|---|
| create_market | `49rJaybpzNxv24cFNYQjpZRsuQfUPCZ5ZM4F3Y8Ybrn8cXGXT7X9YMfu8sMwAUaq5PcR5vWdKPfT8hNBp1M1fsYG` |
| deposit A -> Home 40 | `5YcdPUC5NKmZihSzcD1aorgyMZmhcHbkPvzvJTFG6NyivAB6Tg5aLV7yqm4jkRMbKTkp8q2zerSSK5z3kxcrohcZ` |
| deposit B -> Away 60 | `62QsH5jKsAiVyhujnZ3aK9ycvzyc3RDw92x8cj1X1vjvZZwQySeqeFLvb7TVpfu2UXsVSUT9pLiwBvsKTuZ6V4E2` |
| resolve (Home wins) | `3wsdyUxCr57yT9Lx1P7UZb9y2sUFywCBpo5g8dwNn9hfNqKYpNd2FcwiLxXq8o3jxJS8GyNzRj623e9ivJbhKeoM` |
| A claims (winner) | `5KFsUjZBVrLex5Ufzi8TTTcSJpn7YbBEMuL2KDU2uo66ds47E6at6pz9GQm15tbRohCuniwnSdmTnZ3ZC6HtYNC` |
| B claims (loser) | `5X1hB2SAUWMj7kNWnQYRreAp98RVQvq6Ca5vNLphfYg6ZedVtg4r6QixfawEMQ9xQf59muJpEXdKPPFcZuaBcD3s` |

**Result:** receipt outcome `Home`, `reverify() -> true`, final balances userA=158, userB=40,
vault=2 (the 2% protocol fee) — exactly the settlement math the rulebook specifies. Exit 0.

Re-run: `cd tests-devnet && bun install && bun run smoke.ts` (uses a fresh fixture id each run).

## LIVE Tx LINE settlement — a real fixture settled against the on-chain Merkle feed — PASSED

The full real path, no mocks: `tests-devnet/txline-activate.ts` (4-step free World Cup activation) +
`tests-devnet/txline-settle.ts` (settle against the **real** `Txoracle` devnet
`6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`).

- **Activation:** on-chain `subscribe(1, 4)` tx `2hnw1aAkGN4RRqfzRyJiDUEKCq1BnuH9Wm7X6vRet2ozvcZJy1ngU16Fu7NBHoV3rpmKWUdRs1PYgJ2c1C5w778C`,
  then guest JWT + wallet-signed message + `/api/token/activate` -> live API token. Data pulled from
  `https://txline-dev.txodds.com/api`.
- **Fixture 18192996** (real): authenticated score **home 2 - 3 away** via live `stat-validation`
  Merkle proofs (base keys 1/2). Daily root PDA `CMtVGDyWsZ4u3yeYeyC9yxNzzyvwco6Jgtd9ubRJWCGV`
  exists on devnet (9232 bytes).
- **Two-step resolve** (both proofs won't fit one 1232-byte tx):
  - `attest_home` tx `F3KGfUR9HT3divbERGQT85KDZg2VsTN2DkFmDKMHgpuWbGv9HTwu5uVBxMMrT37EyFP2wPtmvy3CwrbC7abUN1U` — CPI `validate_stat` authenticates home goals against the real on-chain root.
  - `resolve` tx `2AHVsDyz5atZVLAYfzJeZy795uT3yKj44sugGoL3K4EKMHYLGvx4w6kLFpHDCLztet2oAkJt1GUZRfEY582ewrmA` — authenticates away goals, rulebook resolves.
- **Result:** receipt outcome **Away** (2 < 3), `reverify() -> true`, winner (Away pool) paid pro-rata.
  Needs a 1.4M compute-unit budget (Merkle verification is CU-heavy).

Re-run: `bun run txline-activate.ts && bun run txline-settle.ts 18192996 770`.

## Remaining
Mainnet (real-time L12) run, and a completed-match fixture with a `Completed` status (18192996's feed
score was authenticated live; for a production market pick a finished fixture + real status code).
