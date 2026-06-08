import { sql } from "@/lib/db";
import * as zlib from "zlib";
import { pack } from "tar-stream";

export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";

// Bundle all per-node logs for a run into a single .tar.gz download.
export async function GET(_req: Request, { params }: { params: { id: string } }) {
  const rows = (await sql`
    SELECT node_id, body FROM logs WHERE run_id=${params.id} ORDER BY node_id
  `) as { node_id: string; body: string | null }[];
  if (!rows.length) return new Response("no logs for this run", { status: 404 });

  const p = pack();
  for (const r of rows) p.entry({ name: `${params.id}/${r.node_id}.log` }, r.body ?? "");
  p.finalize();

  const chunks: Buffer[] = [];
  for await (const c of p as any) chunks.push(c as Buffer);
  const gz = zlib.gzipSync(Buffer.concat(chunks));

  return new Response(gz, {
    headers: {
      "Content-Type": "application/gzip",
      "Content-Disposition": `attachment; filename="${params.id}-logs.tar.gz"`,
    },
  });
}
