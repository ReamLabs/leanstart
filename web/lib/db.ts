import { neon } from "@neondatabase/serverless";

// Server-only: importing this module touches DATABASE_URL. Client components
// must import constants/types from "@/lib/fixes", never from here.
const url = process.env.DATABASE_URL;
if (!url) throw new Error("DATABASE_URL is not set");

// `sql` is a tagged-template query function (parameterized, safe).
export const sql = neon(url);

export type { RunRow } from "./fixes";
export { FIX_KEYS, FIX_LABELS } from "./fixes";
