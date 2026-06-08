import { sql } from "@/lib/db";
import { ingestRun, RunFiles } from "@/lib/ingest";
import * as zlib from "zlib";
import { extract } from "tar-stream";
import { Readable } from "stream";

export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";
export const maxDuration = 60;

// Accepts a gzipped tar (produced by `leanstart runs push`) containing one run
// dir: <id>/run.json, <id>/metrics.json, <id>/run.log, <id>/<node>.log
export async function POST(req: Request) {
  const token = (req.headers.get("authorization") || "").replace(/^Bearer\s+/i, "");
  if (!process.env.INGEST_TOKEN || token !== process.env.INGEST_TOKEN) {
    return new Response("unauthorized", { status: 401 });
  }

  const buf = Buffer.from(await req.arrayBuffer());
  let files: { id: string; run: RunFiles };
  try {
    files = await parseTarball(buf);
  } catch (e: any) {
    return new Response(`bad tarball: ${e.message}`, { status: 400 });
  }
  if (!files.id) return new Response("no run dir in tarball", { status: 400 });

  try {
    const r = await ingestRun(sql, files.id, files.run);
    return Response.json({ ok: true, ...r });
  } catch (e: any) {
    return new Response(`ingest failed: ${e.message}`, { status: 500 });
  }
}

function parseTarball(gz: Buffer): Promise<{ id: string; run: RunFiles }> {
  return new Promise((resolve, reject) => {
    const tar = gunzipMaybe(gz);
    const ex = extract();
    let id = "";
    const run: RunFiles = { runJson: null, metricsJson: null, runLog: "", logs: {} };

    ex.on("entry", (header, stream, next) => {
      const parts = header.name.split("/");
      const base = parts[parts.length - 1];
      if (parts.length >= 2 && !id) id = parts[0];
      const chunks: Buffer[] = [];
      stream.on("data", (c: Buffer) => chunks.push(c));
      stream.on("end", () => {
        const body = Buffer.concat(chunks).toString("utf8");
        if (base === "run.json") run.runJson = safeJson(body);
        else if (base === "metrics.json") run.metricsJson = safeJson(body);
        else if (base === "run.log") run.runLog = body;
        else if (base.endsWith(".log")) run.logs[base.replace(/\.log$/, "")] = body;
        next();
      });
      stream.resume();
    });
    ex.on("finish", () => resolve({ id, run }));
    ex.on("error", reject);
    Readable.from(tar).pipe(ex);
  });
}

function gunzipMaybe(buf: Buffer): Buffer {
  // gzip magic 0x1f 0x8b
  if (buf[0] === 0x1f && buf[1] === 0x8b) return zlib.gunzipSync(buf);
  return buf;
}

function safeJson(s: string) {
  try {
    return JSON.parse(s);
  } catch {
    return null;
  }
}
