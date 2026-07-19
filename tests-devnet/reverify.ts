/**
 * PERMISSIONLESS reverify: re-derive a settled market's resolution from a *fresh, unrelated*
 * wallet that never created, deposited, or resolved. reverify() is a read-only Anchor `.view()`
 * (no signer, no fee, no state change) — anyone can re-run the exact rulebook against the on-chain
 * receipt and get back a boolean.
 *
 * Usage: bun run reverify.ts <marketPubkey>
 *   e.g. bun run reverify.ts GaiXEuSBb3spjoptxHCoyScycN4sCy164jCF3jT9v8T3   (the live fixture 18192996 market)
 */
import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { readFileSync } from "fs";

const RPC = "https://api.devnet.solana.com";
const market = new PublicKey(process.argv[2] || "GaiXEuSBb3spjoptxHCoyScycN4sCy164jCF3jT9v8T3");

// A stranger: a keypair with no relationship to this market. reverify() is a read-only view, but
// the simulation's fee-payer account must EXIST on devnet (hold any lamports) — an account devnet
// has never seen returns AccountNotFound. Falls back: stranger.json -> default solana wallet ->
// ephemeral keypair (works once airdropped: `solana airdrop 0.5 <pubkey> -u devnet`).
function loadStranger(): Keypair {
  for (const path of [
    `${process.env.HOME}/.config/solana/stranger.json`,
    `${process.env.HOME}/.config/solana/id.json`,
  ]) {
    try {
      return Keypair.fromSecretKey(Uint8Array.from(JSON.parse(readFileSync(path, "utf8"))));
    } catch {}
  }
  return Keypair.generate();
}
const stranger = loadStranger();

const conn = new Connection(RPC, "confirmed");
const provider = new anchor.AnchorProvider(conn, new anchor.Wallet(stranger), { commitment: "confirmed" });
anchor.setProvider(provider);
const idl = JSON.parse(readFileSync(new URL("../idl/var_settlement.json", import.meta.url), "utf8"));
const program = new anchor.Program(idl, provider);

console.log(`stranger wallet: ${stranger.publicKey.toBase58()} (never touched market ${market.toBase58()})`);
try {
  const verified = await program.methods.reverify().accounts({ market }).view();
  console.log(`\n== reverify (stranger wallet) -> ${verified} ==`);
  process.exit(verified === true ? 0 : 1);
} catch (e: any) {
  const msg = String(e?.message ?? e);
  if (msg.includes("AccountNotFound") || msg.includes("Attempt to debit")) {
    console.error(
      `\nThe fee-payer wallet doesn't exist on devnet yet (simulation needs an existing account).` +
      `\nFix: solana airdrop 0.5 ${stranger.publicKey.toBase58()} -u devnet   # then re-run` +
      `\n(the wallet stays a stranger to the market — it only pays the simulated fee)`);
    process.exit(2);
  }
  throw e;
}
