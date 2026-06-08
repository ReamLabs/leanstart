import { sql } from "@/lib/db";

export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";

export async function GET(
  _req: Request,
  { params }: { params: { id: string; node: string } }
) {
  const rows = await sql`
    SELECT body FROM logs WHERE run_id=${params.id} AND node_id=${params.node}
  `;
  if (!rows.length) return new Response("no such log", { status: 404 });
  return new Response((rows[0] as any).body ?? "", {
    headers: { "Content-Type": "text/plain; charset=utf-8" },
  });
}
