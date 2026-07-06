# VAR — 3-Minute Demo Video Script (Recordable)

**Project:** VAR — Verifiable Automated Resolution
**Event:** Superteam World Cup Hackathon — Track 1: Prediction Markets & Settlement ($18,000)
**Repo:** https://github.com/JuanMarchetto/var-settlement
**Runtime target:** ~180 s. Screen recording + voiceover. Everything shown is real and on devnet.

**Honesty rule (state it in the description, honor it on screen):** no edit removes, reorders, or fabricates real terminal output. Time-compression is allowed but **labeled** — a `cargo kani` bounded model-check is minutes, and two 1.4M-CU resolve transactions plus live API round-trips are not 30-second events, so any sped-up or pre-captured segment carries a caption (`2×, unedited` or `real run — tx signatures on-chain`). The one genuinely live, real-time keystroke on camera is the permissionless `reverify` from a stranger wallet.

---

## Recording setup (read first)

- **Terminal:** dark theme, font **18–20 pt**, ~110–120 columns, generous line height. One prompt visible, scrollback clean.
- **Three panes to pre-open:**
  - **Tab A — Proofs:** `cd /home/marche/var-settlement/crates/rulebook` (runs `cargo kani`, then `cargo test`).
  - **Tab B — Live settlement:** `cd /home/marche/var-settlement/tests-devnet` (the real Tx LINE settle — **pre-warmed**, see below).
  - **Tab C — Permissionless reverify:** `cd /home/marche/var-settlement/tests-devnet` (the stranger-wallet `reverify`, run **live** on camera).
  - **Browser:** Solana Explorer, resolve-tx URL preloaded but not yet revealed (see Assets).
- **Pre-warm before rolling (these are slow and the live API token is short-lived):**
  1. **Kani:** run `cargo kani` once so results are cached; on camera re-run it and let the cached `Complete - 4 …` line land, or speed-ramp the live run and label it `2×, unedited`.
  2. **Tx LINE settle:** run `bun run txline-activate.ts` (fresh live token), then `bun run txline-settle.ts 18192996 770` **before** recording. Tab B on screen is that real run's captured output (caption: `real run — tx signatures on-chain`). The canonical settled market it produces is already on devnet: market PDA `A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL`, resolve tx `4j2u…` — the video anchors every Explorer link and the reverify to **that** market so everything a judge clicks is consistent.
  3. **Stranger wallet (required for the live reverify):** create a keypair that never touches the market and **fund it minimally** — `solana-keygen new -o ~/.config/solana/stranger.json --no-bip39-passphrase` then `solana airdrop 0.02 ~/.config/solana/stranger.json -u devnet`. `reverify` is a read-only `.view()`, but Solana still simulates against a fee-payer that must **exist on-chain** — an unfunded stranger returns `AccountNotFound`, not `true`. The 0.02 SOL only makes the wallet exist; it never touches the market, so "never touched this market" holds. (Verified: unfunded → `AccountNotFound`; funded → `reverify (stranger wallet) -> true`.)
- **Caption `770`** the first time it appears: *"770 = Tx LINE feed sequence #"* — it's the feed's update index for the fixture, needed to fetch the right stat-validation snapshot.
- **Pace:** ~2–2.5 words/sec. Let the `cargo kani` tally, the two tx signatures, and the stranger `reverify -> true` sit on screen — the pauses are the proof.

---

## Timecode table (0:00 – 3:00)

| Time | On screen | Voiceover (exact narration) | Terminal command / action |
|---|---|---|---|
| **0:00–0:10** | Problem card. At **~0:08** it swaps to: **"VAR — settlement anyone can re-run."** | "This year, a reported sixty-million-dollar prediction market resolved against a filed SEC document — because the biggest token holders voted that way. That's the model working as designed." | Static card. No terminal yet. |
| **0:10–0:22** | Title card: **"VAR — Verifiable Automated Resolution"** / "World Cup 1X2 settlement on Solana. No token vote. No dispute bond. No arbiter." | "This is VAR — Verifiable Automated Resolution. World Cup match settlement with no token vote, no dispute bond, no arbiter — just a proof anyone can re-run." | Cut to **Tab A**, prompt ready. |
| **0:22–0:48** | `cargo kani` running, then the summary: **"Complete - 4 successfully verified harnesses, 0 failures, 4 total."** (Caption if sped up: `2×, unedited`.) | "The core is a pure Rust rulebook — not just tested, formally proven. Four Kani harnesses, exhaustively model-checked, not sampled: totality and fail-closed, value conservation, settlement fail-closed, and determinism. This proves the rulebook cannot mint money, cannot panic, and cannot give two different answers for the same match." | Type `cargo kani`. Let the four `VERIFICATION:- SUCCESSFUL` lines and the `Complete - 4 … 4 total.` tally land. |
| **0:48–1:05** | `cargo test` scrolling to `test result: ok`. Briefly highlight `06_penalties-draw-argentina-france-final`. | "Underneath it: twelve real World Cup vectors — including the 2022 final, two-two, decided on penalties, which settles Draw on the ninety-minute result — plus twelve thousand property-tested payout cases. All green." | Type `cargo test -p rulebook`. Let `test result: ok` show. |
| **1:05–1:30** | **Tab B** (real run, captioned `real run — tx signatures on-chain`). Output shows `== fetch REAL proofs for fixture 18192996 seq 770 ==` and `authenticated score: home 2 - 3 away`. Caption `770 = Tx LINE feed sequence #`. | "Now the real thing, no mock. This settles a real fixture against Tx LINE's live on-chain feed. The SDK pulls the final goal counts and a Merkle proof for each side, then hands the claimed score to the program — home two, away three." | Reveal Tab B: the captured `bun run txline-settle.ts 18192996 770`. Let the authenticated scoreline land. |
| **1:30–2:00** | `attest_home tx: 53dk…` then `resolve tx: 4j2u…`, then `receipt: outcome=Away paidAsRefund=false …`, then `REAL SETTLEMENT PASSED  (resolve tx: 4j2u…)`. Callout: real Txoracle `6pW64gN1…wyP2J`, ~1.4M CU. | "resolve doesn't take my word for the score. It CPIs into the real Tx LINE oracle and proves each goal count against its Merkle-committed daily root — home goals in attest_home, away goals in resolve. Two transactions, because both proofs won't fit one. Match status is a separate input the Kani-proven rulebook fail-closes: anything but a completed result refunds everyone. Two proven against three — outcome Away. Nobody signed it. Nobody voted." | Let both signatures and the `REAL SETTLEMENT PASSED` line print. |
| **2:00–2:28** | **Tab C — LIVE.** Prints `stranger wallet: WQMF…` then, big: **`== reverify (stranger wallet) -> true ==`**. Optional green-check overlay. | "Here's what makes it trustless. This is a fresh wallet that never created, funded, or resolved this market. It calls reverify — a read-only view, no signer, no fee — and re-derives the entire outcome and payout from scratch against the on-chain receipt. One bit off and it returns false. It returns true." | Type live: `bun run reverify.ts A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL`. Hold on `-> true` for 2 s. |
| **2:28–2:45** | Cut to **browser**: Solana Explorer, the **`resolve` tx** page (devnet), instruction list showing the CPI into Txoracle and the compute budget; then flash the market/program page. | "And you don't have to take my word for any of it. Here's that resolve transaction on Solana Explorer — the CPI into the real Txoracle, on a program live on devnet right now. Open it. Re-run it yourself." | Reveal preloaded Explorer (resolve tx `4j2u…`). Scroll to the inner CPI instruction. |
| **2:45–3:00** | End card: repo URL + `cargo kani && cargo test -p rulebook` + "No token vote. No dispute. No arbiter." + "Track 1 submission." | "Formally proven, permissionlessly re-verifiable, settled straight off the oracle's own Merkle root — no token vote, no dispute bond, no arbiter can override it. Clone the repo. Run the proofs. That's VAR." | Static end card. Fade. |

---

## Assets to have on screen (copy-paste ready)

**Commands (exact):**
```bash
# Tab A (cd crates/rulebook)
cargo kani            # -> "Complete - 4 successfully verified harnesses, 0 failures, 4 total."
cargo test -p rulebook

# Tab B (cd tests-devnet) — pre-warm BEFORE recording (token is short-lived)
bun run txline-activate.ts
bun run txline-settle.ts 18192996 770     # 770 = Tx LINE feed sequence #

# Tab C (cd tests-devnet) — run LIVE on camera, from the funded stranger wallet
bun run reverify.ts A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL
```

**On-chain addresses:**
- var_settlement program (devnet): `AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk`
- Real Txoracle / Tx LINE (devnet): `6pW64gN1s2uqjHkn1unFeEjAwJkPGHoppGvS715wyP2J`
- Canonical settled **market PDA** (fixture 18192996 — what `reverify` targets): `A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL`
- Daily-scores-roots PDA (fixture's day): `CMtVGDyWsZ4u3yeYeyC9yxNzzyvwco6Jgtd9ubRJWCGV`
- Stranger wallet (fund with 0.02 SOL, never touches the market): `WQMF7mLsD4CJ5UKCGHJFCMCdjce593xtjSnRi78gmv1`

**Transactions to show (Explorer, `?cluster=devnet`):**
- `attest_home` (home goals authenticated): `53dkuaseF6pAD71WDAaPUzwEQFQ6keWgRuafVM8DBqyvBZqWMwQg3GAtzbuJP5fSFYJ1rxpDKbE7HMK1AXtfbsws`
  https://explorer.solana.com/tx/53dkuaseF6pAD71WDAaPUzwEQFQ6keWgRuafVM8DBqyvBZqWMwQg3GAtzbuJP5fSFYJ1rxpDKbE7HMK1AXtfbsws?cluster=devnet
- `resolve` (MONEY SHOT — away goals authenticated, rulebook resolves):
  `4j2ukzmW8rJNMAuCiyyKaiqksviB6mZS26e4FSfFuhBynV5mQXv8DMasLJUopv3XUC4BsFHQxPNPLPoj69oVtnyC`
  https://explorer.solana.com/tx/4j2ukzmW8rJNMAuCiyyKaiqksviB6mZS26e4FSfFuhBynV5mQXv8DMasLJUopv3XUC4BsFHQxPNPLPoj69oVtnyC?cluster=devnet
- Program page: https://explorer.solana.com/address/AepSNpDzMUdBgjxA9irxxL7NTQHxXtDVq6rnqq17Lxk?cluster=devnet
- Market page (the one `reverify` re-derives): https://explorer.solana.com/address/A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL?cluster=devnet

**`reverify.ts` (already in `tests-devnet/`, source shown for self-containment):**
```ts
// bun run reverify.ts <marketPubkey>  — read-only .view() from a stranger wallet (no signer, no fee)
import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { readFileSync } from "fs";

const RPC = "https://api.devnet.solana.com";
const market = new PublicKey(process.argv[2] || "A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL");
const stranger = Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(readFileSync(`${process.env.HOME}/.config/solana/stranger.json`, "utf8"))));
const conn = new Connection(RPC, "confirmed");
const provider = new anchor.AnchorProvider(conn, new anchor.Wallet(stranger), { commitment: "confirmed" });
anchor.setProvider(provider);
const idl = JSON.parse(readFileSync(new URL("../idl/var_settlement.json", import.meta.url), "utf8"));
const program = new anchor.Program(idl, provider);
console.log(`stranger wallet: ${stranger.publicKey.toBase58()} (never touched market ${market.toBase58()})`);
const verified = await program.methods.reverify().accounts({ market }).view();
console.log(`\n== reverify (stranger wallet) -> ${verified} ==`);
process.exit(verified === true ? 0 : 1);
```

**Facts to keep straight on camera:** fixture `18192996`, feed sequence `770`, authenticated **goal counts** home `2` – away `3` (match **status is a caller-supplied input** the rulebook fail-closes — do not imply status is oracle-proven), outcome **Away**, `reverify -> true`, real Txoracle `6pW64gN1…wyP2J`, `resolve` needs a **1.4M compute-unit** budget (Merkle verification is CU-heavy).

---

## Fallback (only if the live Tx LINE token/feed is flaky at shoot time)

- **Line 1 — mock end-to-end:** run `cd tests-devnet && bun run smoke.ts` — full `create → deposit → resolve (Home 2–0, CPI validate_stat) → reverify → claim`, final balances **158 / 40 / 2** (the 2% fee), outcome Home. Caption on screen: *"mock Txoracle stand-in — real Tx LINE run in DEPLOYMENTS.md."*
- **Line 2 — pre-recorded real run:** splice the captured `bun run txline-settle.ts 18192996 770` (label it *"pre-recorded, tx signatures on-chain — see DEPLOYMENTS.md"*). The non-negotiable floor is: the Kani `4 total` line, one real settled market, and the live stranger-wallet `reverify (stranger wallet) -> true`.
- **Stranger `reverify` is the safest live moment even if the feed is down** — it re-derives the already-settled market `A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL` from an on-chain receipt, independent of Tx LINE availability. Keep it in every cut.
