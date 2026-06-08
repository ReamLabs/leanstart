"use client";

import { useEffect, useState } from "react";

export default function LogViewer({ id, nodes }: { id: string; nodes: string[] }) {
  const [active, setActive] = useState(nodes[0] || "");
  const [body, setBody] = useState("loading…");

  useEffect(() => {
    if (!active) return;
    setBody("loading…");
    fetch(`/api/runs/${id}/log/${active}`)
      .then((r) => (r.ok ? r.text() : "no log"))
      .then(setBody);
  }, [id, active]);

  if (!nodes.length)
    return (
      <div className="cardbody">
        <span className="faint">No per-node logs captured for this run.</span>
      </div>
    );

  return (
    <div>
      <div className="tabs">
        {nodes.map((n) => (
          <div
            key={n}
            className={`tab ${n === active ? "active" : ""}`}
            onClick={() => setActive(n)}
          >
            {n}
          </div>
        ))}
        <span style={{ flex: 1 }} />
        <a className="dlbtn" href={`/api/runs/${id}/log/${active}?download=1`} download>
          ↓ {active}.log
        </a>
        <a className="dlbtn primary" href={`/api/runs/${id}/logs`} download>
          ↓ Download all
        </a>
      </div>
      <pre className="log">{body}</pre>
    </div>
  );
}
