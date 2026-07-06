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

## Next (needs the live Tx LINE feed)
Point a market at the real `Txoracle` (devnet `6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`),
complete the 4-step activation, fetch a real `stat-validation` proof, and settle one real World Cup
fixture against the on-chain daily Merkle root.
