# Tx LINE API/docs feedback

Notes from actually integrating against Tx LINE (`Txoracle`) while building VAR — 4-step activation,
`validate_stat` CPI, PDA derivation, the works. Written to be useful to the TxODDS team, not to
complain. Every point below is something we hit while implementing
`packages/sdk/src/txline.ts` and `programs/var-settlement/src/lib.rs::txoracle_cpi`, and worked
around, but shouldn't have had to work around.

## 1. The 4-step activation flow has real friction for an on-chain-first builder

Getting from "I have a wallet" to "I have an `X-Api-Token`" is: send an on-chain `subscribe`
transaction → `POST /auth/guest/start` for a JWT → sign a composite string
(`${txSig}:${leagues}:${jwt}`) with Ed25519 → `POST /api/token/activate`. That's a reasonable design
for tying an API credential to a wallet, but it means every new integrator has to:

- Know the exact message format to sign (`${txSig}:${leagues}:${jwt}`) — this isn't obvious from a
  first read and cost us a spec-reading pass to get right (see `spec.md` §2 for the format we
  landed on).
- Handle two different token types (`jwt` for the guest step, `apiToken` for actual data calls) with
  two different headers on subsequent requests (`Authorization: Bearer <jwt>` *and*
  `X-Api-Token: <token>`, both, every call).
- Write the on-chain `subscribe(serviceLevel, durationWeeks)` transaction against the `Txoracle`
  IDL themselves, since there's no published TS client for the subscribe step — we have a stub
  (`subscribe()` in `packages/sdk/src/txline.ts`) that intentionally throws until it's wired against
  a funded wallet, precisely because the account list for that instruction isn't spelled out
  anywhere except "read the IDL."

**Ask:** a one-page "activation in 4 curl commands + 1 `web3.js` snippet" doc, or a thin published
`@txodds/txline-sdk` that wraps `subscribe` + the guest/activate handshake, would save every
hackathon team (and every future integrator) the same afternoon we spent reconstructing it from the
IDL and docs.

## 2. The mainnet-only real-time tier makes devnet-first development second-class

L1 (World Cup/Friendlies, 60s-delayed) works on devnet. L12 (real-time, sub-second) is mainnet-only.
That's a defensible business decision, but it means anyone building and testing against devnet — the
normal, safe place to iterate before touching real funds — is permanently capped at a 60-second-old
view of the world, with no devnet path to test the low-latency behavior your flagship tier actually
offers. We didn't hit this as a blocker (VAR settles post-match, not live-in-play, so 60s delay is
irrelevant to us) but a team building an in-play or live-odds product would find out the hard way
that their whole devnet test suite runs against a materially different latency profile than
production. A documented, rate-limited "L12-shaped but delayed" devnet tier would close that gap.

## 3. The nested Merkle proof model is powerful but underdocumented at the "why" level

`validate_stat` takes a `fixture_proof` (stat → fixture summary) *and* a separate `main_tree_proof`
(fixture summary → daily root) — a two-level nested proof, not a single flat Merkle path. Once we
understood it, the design makes sense (it lets you commit fixtures independently within a day and
batch them into one daily root). But the docs describe the fields without walking through *why*
there are two proof arrays instead of one, which meant we had to reverse-engineer the tree shape
from the IDL's struct layout (`ScoresBatchSummary { fixture_id, update_stats,
events_sub_tree_root }` nested inside the day's tree) rather than read it in one place. A single
diagram — stat leaf → `event_stat_root` → fixture `events_sub_tree_root` → daily main root, with the
two proof arrays labeled against it — would have cut our integration time by a real margin.

## 4. Daily-root publish timing is the single most important undocumented number

The entire settlement-timing design of VAR (`RESOLVE_GRACE_SECS = 7 days` in
`programs/var-settlement/src/lib.rs`, see `docs/AUDIT.md`) is a guess, because we could not find a
documented SLA for "how long after a match ends does the root covering that update actually publish
on-chain." We know roots publish per epoch-day and that verification is against the day's committed
root (not per-tick), but "minutes later" versus "up to 24 hours later" versus "next calendar day
regardless of match end time" are three very different numbers that change how every downstream
settlement contract should be designed. This is the single piece of information we'd most want
published explicitly (with real observed percentiles, not just a target) — every team settling
anything off this feed needs it, not just us.

## 5. `validate_stat`'s `.view()`-shaped semantics vs. its CPI reality

`validate_stat` reads like a pure predicate check ("does this stat satisfy this comparison") — the
kind of function you'd expect to call as a read-only `.view()` / simulated call from a client, the
way you'd probe an oracle before committing to an on-chain action. In practice, because VAR needs
the *result* of that check to gate on-chain state transitions (whether `resolve` is allowed to
proceed), we have to CPI into it for real and read the answer back via Solana's return-data
mechanism (`get_return_data()` after `invoke()`), not simulate it client-side. That's the correct
design for a settlement contract, but the docs read like a query API first and a CPI primitive
second — flipping the framing (lead with "this is designed to be CPI'd into for on-chain gating;
client-side simulation is also possible for previewing") would have saved us the CPI-return-data
plumbing being a surprise rather than an expected step.

## 6. Doc/nav slug quirks

Smaller, but worth flagging: a few documentation pages under
`txline.txodds.com/documentation/*` use slugs that don't match the section headers shown once you
land on the page (a link titled one way in the nav resolves to a page whose H1 says something
slightly different, e.g. a stat-key glossary reachable from a link that reads like it's about
authentication). Not blocking, but it cost a few dead-end clicks while cross-referencing the IDL
against the docs to confirm `validate_stat`'s exact signature. A slug/heading consistency pass would
help anyone doing that same cross-reference.

---

None of this blocked us — the exact interface we needed (`validate_stat`, the PDA seeds, the
activation flow) is documented well enough to build against, which is why VAR exists. This is meant
as "here's where the next team will lose an afternoon," not a complaint about the feed itself.
