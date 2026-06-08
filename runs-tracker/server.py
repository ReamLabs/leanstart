#!/usr/bin/env python3
"""
leanstart run tracker — zero-dependency web server (Python stdlib only).

Scans the `output/runs/<timestamp>/` tree that `leanstart run` already produces
and exposes every run + the information collected during it:
  - the spec (clients / images / subnets, parsed from run.log)
  - per-node logs (raw, viewable)
  - a finalization timeline (head / justified / finalized slot over time,
    parsed from ream's "REAM's CHAIN STATUS" blocks and ethlambda status lines)
  - the run outcome (max finalized slot, did-it-finalize, duration)
  - the genesis config if it was snapshotted into the run dir

Run:  python3 runs-tracker/server.py [--port 8099] [--runs ./output/runs]
Then open http://localhost:8099
"""
import argparse
import json
import os
import re
import html
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlparse, unquote

ANSI = re.compile(r"\x1b\[[0-9;]*m")
TS = re.compile(r"(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})")

# ream "REAM's CHAIN STATUS" block fields
RE_HEAD = re.compile(r"Head Slot:\s*(\d+)")
RE_JUST = re.compile(r"Latest Justified:\s*Slot\s*(\d+)")
RE_FIN = re.compile(r"Latest Finalized:\s*Slot\s*(\d+)")
# ethlambda inline status fields
RE_E_HEAD = re.compile(r"our_head_slot=(\d+)")
RE_E_FIN = re.compile(r"our_finalized_slot=(\d+)")
RE_E_FINAL = re.compile(r"Finalized[:=]\s*(\d+)")

HERE = os.path.dirname(os.path.abspath(__file__))
RUNS_DIR = os.path.join(HERE, "..", "output", "runs")


def strip(line):
    return ANSI.sub("", line)


def parse_spec(run_log_path):
    """Extract the devnet spec (summary + client/image lines) from run.log."""
    spec = {"summary": "", "clients": [], "command": ""}
    if not os.path.exists(run_log_path):
        return spec
    with open(run_log_path, errors="replace") as f:
        for raw in f:
            line = strip(raw).rstrip("\n")
            if line.startswith("Devnet:"):
                spec["summary"] = line
            m = re.match(r"\s+([a-z]+) x(\d+) \((.+)\)\s*$", line)
            if m:
                spec["clients"].append(
                    {"name": m.group(1), "count": int(m.group(2)), "image": m.group(3)}
                )
            if "--devnet5" in line or "leanstart run" in line:
                spec["command"] = line.strip()
    return spec


def parse_timeline(log_path):
    """Parse (timestamp, head, justified, finalized) points from one node log.

    Handles ream's multi-line CHAIN STATUS block (fields share the timestamp of
    the line that opened the block) and ethlambda's single-line status fields.
    """
    points = []
    last_ts = None
    pending = {}  # ream block accumulator
    if not os.path.exists(log_path):
        return points
    with open(log_path, errors="replace") as f:
        for raw in f:
            line = strip(raw)
            tsm = TS.search(line)
            if tsm:
                last_ts = tsm.group(1)
            # ream block fields
            mh, mj, mfn = RE_HEAD.search(line), RE_JUST.search(line), RE_FIN.search(line)
            if mh:
                pending = {"ts": last_ts, "head": int(mh.group(1))}
            if mj and pending:
                pending["justified"] = int(mj.group(1))
            if mfn and pending:
                pending["finalized"] = int(mfn.group(1))
                points.append(
                    {
                        "ts": pending.get("ts"),
                        "head": pending.get("head"),
                        "justified": pending.get("justified"),
                        "finalized": pending["finalized"],
                    }
                )
                pending = {}
            # ethlambda inline status
            eh, ef = RE_E_HEAD.search(line), RE_E_FIN.search(line)
            if eh and ef:
                points.append(
                    {
                        "ts": last_ts,
                        "head": int(eh.group(1)),
                        "justified": None,
                        "finalized": int(ef.group(1)),
                    }
                )
    return points


def node_logs(run_dir):
    return sorted(
        n
        for n in os.listdir(run_dir)
        if n.endswith(".log") and n != "run.log"
    )


def run_summary(run_id):
    run_dir = os.path.join(RUNS_DIR, run_id)
    logs = node_logs(run_dir)
    best = {"finalized": 0, "head": 0, "justified": 0, "log": None}
    for lg in logs:
        pts = parse_timeline(os.path.join(run_dir, lg))
        if pts:
            mx = max(pts, key=lambda p: p["finalized"] or 0)
            if (mx["finalized"] or 0) >= best["finalized"]:
                best = {
                    "finalized": mx["finalized"] or 0,
                    "head": mx["head"] or 0,
                    "justified": mx["justified"] or 0,
                    "log": lg,
                }
    spec = parse_spec(os.path.join(run_dir, "run.log"))
    st = os.stat(run_dir)
    return {
        "id": run_id,
        "mtime": st.st_mtime,
        "nodes": logs,
        "summary": spec["summary"],
        "clients": spec["clients"],
        "max_finalized": best["finalized"],
        "max_head": best["head"],
        "finalized": best["finalized"] > 0,
    }


def list_runs():
    if not os.path.isdir(RUNS_DIR):
        return []
    out = []
    for d in os.listdir(RUNS_DIR):
        p = os.path.join(RUNS_DIR, d)
        if os.path.isdir(p):
            try:
                out.append(run_summary(d))
            except Exception as e:  # never let one bad run break the list
                out.append({"id": d, "error": str(e), "mtime": os.stat(p).st_mtime})
    out.sort(key=lambda r: r.get("mtime", 0), reverse=True)
    return out


def run_detail(run_id):
    run_dir = os.path.join(RUNS_DIR, run_id)
    logs = node_logs(run_dir)
    timelines = {lg: parse_timeline(os.path.join(run_dir, lg)) for lg in logs}
    spec = parse_spec(os.path.join(run_dir, "run.log"))
    genesis = None
    for cand in ("config.yaml", os.path.join("genesis", "config.yaml")):
        gp = os.path.join(run_dir, cand)
        if os.path.exists(gp):
            with open(gp, errors="replace") as f:
                genesis = f.read()[:20000]
            break
    return {
        "id": run_id,
        "spec": spec,
        "nodes": logs,
        "timelines": timelines,
        "genesis": genesis,
        "summary": run_summary(run_id),
    }


class Handler(BaseHTTPRequestHandler):
    def log_message(self, *a):
        pass

    def _send(self, code, body, ctype="application/json"):
        if isinstance(body, (dict, list)):
            body = json.dumps(body).encode()
        elif isinstance(body, str):
            body = body.encode()
        self.send_response(code)
        self.send_header("Content-Type", ctype)
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self):
        path = unquote(urlparse(self.path).path)
        if path == "/" or path == "/index.html":
            with open(os.path.join(HERE, "index.html")) as f:
                return self._send(200, f.read(), "text/html; charset=utf-8")
        if path == "/api/runs":
            return self._send(200, list_runs())
        m = re.match(r"^/api/runs/([^/]+)$", path)
        if m:
            rid = m.group(1)
            if not os.path.isdir(os.path.join(RUNS_DIR, rid)):
                return self._send(404, {"error": "no such run"})
            return self._send(200, run_detail(rid))
        m = re.match(r"^/api/runs/([^/]+)/log/([^/]+)$", path)
        if m:
            rid, name = m.group(1), m.group(2)
            lp = os.path.join(RUNS_DIR, rid, name)
            # contain to run dir
            if not os.path.abspath(lp).startswith(os.path.abspath(os.path.join(RUNS_DIR, rid))):
                return self._send(403, {"error": "forbidden"})
            if not os.path.exists(lp):
                return self._send(404, {"error": "no such log"})
            with open(lp, errors="replace") as f:
                txt = strip(f.read())
            return self._send(200, txt, "text/plain; charset=utf-8")
        return self._send(404, {"error": "not found"})


def main():
    global RUNS_DIR
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8099)
    ap.add_argument("--runs", default=RUNS_DIR)
    args = ap.parse_args()
    RUNS_DIR = os.path.abspath(args.runs)
    print(f"leanstart run tracker: scanning {RUNS_DIR}")
    print(f"open http://localhost:{args.port}")
    ThreadingHTTPServer(("0.0.0.0", args.port), Handler).serve_forever()


if __name__ == "__main__":
    main()
