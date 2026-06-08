/**
 * Backfill importer: scan ../output/runs/<ts>/ and load every run into Neon via
 * the shared ingest core (lib/ingest.ts). New runs carry run.json (+metrics.json
 * +logs); old runs fall back to run.log spec + log-derived timelines.
 *
 * Usage:  npx tsx scripts/import-runs.ts [runsDir]
 *         (DATABASE_URL read from web/.env.local)
 */
import { neon } from "@neondatabase/serverless";
import * as fs from "fs";
import * as path from "path";
import { ingestRun, RunFiles } from "../lib/ingest";

function loadEnvLocal() {
  const p = path.join(__dirname, "..", ".env.local");
  if (!fs.existsSync(p)) return;
  for (const line of fs.readFileSync(p, "utf8").split("\n")) {
    const m = line.match(/^\s*([A-Z_]+)\s*=\s*(.*)\s*$/);
    if (m && !process.env[m[1]]) process.env[m[1]] = m[2];
  }
}
loadEnvLocal();
const DATABASE_URL = process.env.DATABASE_URL;
if (!DATABASE_URL) throw new Error("DATABASE_URL not set (web/.env.local)");
const sql = neon(DATABASE_URL);

const RUNS_DIR =
  process.argv[2] || path.join(__dirname, "..", "..", "output", "runs");

function readRunFiles(dir: string): RunFiles {
  const read = (f: string) => {
    try {
      return fs.readFileSync(path.join(dir, f), "utf8");
    } catch {
      return null;
    }
  };
  const readJson = (f: string) => {
    const s = read(f);
    try {
      return s ? JSON.parse(s) : null;
    } catch {
      return null;
    }
  };
  const logs: Record<string, string> = {};
  for (const n of fs.readdirSync(dir)) {
    if (n.endsWith(".log") && n !== "run.log") {
      logs[n.replace(/\.log$/, "")] = read(n) || "";
    }
  }
  return {
    runJson: readJson("run.json"),
    metricsJson: readJson("metrics.json"),
    runLog: read("run.log") || "",
    logs,
  };
}

async function main() {
  const ids = fs
    .readdirSync(RUNS_DIR)
    .filter((d) => d !== "latest" && fs.statSync(path.join(RUNS_DIR, d)).isDirectory())
    .sort();
  console.log(`Importing ${ids.length} runs from ${RUNS_DIR}`);
  let ok = 0;
  for (const id of ids) {
    try {
      const r = await ingestRun(sql, id, readRunFiles(path.join(RUNS_DIR, id)));
      console.log(
        `  ${r.id}  [${r.status}]  finalized=${r.maxFin}  samples=${r.samples}  logs=${r.logs}`
      );
      ok++;
    } catch (e: any) {
      console.error(`  FAIL ${id}: ${e.message}`);
    }
  }
  console.log(`Done: ${ok}/${ids.length} imported.`);
}

main();
