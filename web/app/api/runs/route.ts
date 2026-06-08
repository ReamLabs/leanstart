import { NextResponse } from "next/server";
import { sql } from "@/lib/db";

export const dynamic = "force-dynamic";
export const fetchCache = "force-no-store";

// Returns all runs with lightweight fields; filtering + faceting happen
// client-side (the dataset is small — tens to low hundreds of runs).
export async function GET() {
  const rows = await sql`
    SELECT run_id, started_at, captured_at, status, client, image, clients, devnet,
           flags, outcome, source
    FROM runs
    ORDER BY run_id DESC
  `;
  return NextResponse.json({ runs: rows });
}
