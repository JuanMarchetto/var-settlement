/**
 * Explore the live Tx LINE data API (devnet) with activated creds: list fixtures, inspect a
 * completed fixture's score stats, and fetch a real stat-validation Merkle proof.
 */
import { readFileSync } from "fs";

const API = "https://txline-dev.txodds.com/api";
const { jwt, apiToken } = JSON.parse(readFileSync(new URL("./.txline-creds.json", import.meta.url), "utf8"));
const H = { Authorization: `Bearer ${jwt}`, "X-Api-Token": apiToken, "Accept-Encoding": "gzip" };

async function get(path: string): Promise<any> {
  const r = await fetch(`${API}${path}`, { headers: H });
  const raw = await r.text();
  if (!r.ok) return { __err: r.status, body: raw.slice(0, 300) };
  try { return JSON.parse(raw); } catch { return { __text: raw.slice(0, 400) }; }
}

async function main() {
  const arg = process.argv[2];
  if (arg) {
    const fid = Number(arg);
    const seq = process.argv[3] ? Number(process.argv[3]) : undefined;
    console.log(`== snapshot ${fid} ==`);
    const snap = await get(`/scores/snapshot/${fid}?asOf=${Date.now()}`);
    const row = Array.isArray(snap) ? snap[0] : snap;
    console.log(`  GameState=${row?.GameState} StatusId=${row?.StatusId} Seq=${row?.Seq}`);
    console.log(`  P1.Total=${JSON.stringify(row?.Score?.Participant1?.Total)} P2.Total=${JSON.stringify(row?.Score?.Participant2?.Total)}`);
    const s = seq ?? row?.Seq;
    for (const statKey of [1, 2]) {
      console.log(`\n== stat-validation fixtureId=${fid} seq=${s} statKey=${statKey} ==`);
      const v = await get(`/scores/stat-validation?fixtureId=${fid}&seq=${s}&statKey=${statKey}`);
      console.log(JSON.stringify(v, null, 1).slice(0, 1400));
    }
    return;
  }
  console.log("== fixtures/snapshot (list) ==");
  const fx = await get(`/fixtures/snapshot`);
  const arr = Array.isArray(fx) ? fx : fx?.fixtures || [];
  console.log(`  ${arr.length} fixtures`);
  for (const f of arr.slice(0, 30)) {
    console.log(`  ${f.FixtureId ?? f.fixtureId}  state=${f.GameState ?? f.gameState ?? "?"}  status=${f.StatusId ?? "?"}  ${f.Participant1Id ?? ""} vs ${f.Participant2Id ?? ""}`);
  }
  if (arr.length === 0) console.log(JSON.stringify(fx, null, 1).slice(0, 1200));
}

main().catch((e) => { console.error(e?.message || e); process.exit(1); });
