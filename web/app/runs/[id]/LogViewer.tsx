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

  if (!nodes.length) return <p className="muted">No per-node logs captured.</p>;
  return (
    <div>
      <div className="btnrow">
        {nodes.map((n) => (
          <button
            key={n}
            className={`btn ${n === active ? "active" : ""}`}
            onClick={() => setActive(n)}
          >
            {n}
          </button>
        ))}
      </div>
      <pre className="log">{body}</pre>
    </div>
  );
}
