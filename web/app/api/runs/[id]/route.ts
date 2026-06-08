import { NextResponse } from "next/server";
import { sql } from "@/lib/db";

export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";

export async function GET(_req: Request, { params }: { params: { id: string } }) {
  const runs = await sql`SELECT * FROM runs WHERE run_id=${params.id}`;
  if (!runs.length) return NextResponse.json({ error: "not found" }, { status: 404 });

  // Pull samples and reshape into { metric: { pod: [[t,v]...] } } for charting.
  const rows = await sql`
    SELECT metric, pod, t, v FROM samples WHERE run_id=${params.id} ORDER BY t
  `;
  const series: Record<string, Record<string, [number, number][]>> = {};
  for (const r of rows as any[]) {
    (series[r.metric] ??= {});
    (series[r.metric][r.pod] ??= []).push([Number(r.t), Number(r.v)]);
  }

  const logs = await sql`SELECT node_id FROM logs WHERE run_id=${params.id} ORDER BY node_id`;

  return NextResponse.json({
    run: runs[0],
    series,
    logNodes: (logs as any[]).map((l) => l.node_id),
  });
}
