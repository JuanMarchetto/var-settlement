/**
 * PERMISSIONLESS reverify: re-derive a settled market's resolution from a *fresh, unrelated*
 * wallet that never created, deposited, or resolved. reverify() is a read-only Anchor `.view()`
 * (no signer, no fee, no state change) — anyone can re-run the exact rulebook against the on-chain
 * receipt and get back a boolean.
 *
 * Usage: bun run reverify.ts <marketPubkey>
 *   e.g. bun run reverify.ts A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL   (the live fixture 18192996 market)
 */
import * as anchor from "@coral-xyz/anchor";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { readFileSync } from "fs";

const RPC = "https://api.devnet.solana.com";
const market = new PublicKey(process.argv[2] || "A81iUQpYd5HuQvkyvpB8YjpvMQwVP8L7xuwak3a9TNYL");

// A stranger: a keypair with no relationship to this market. Read-only view => needs no funds.
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
