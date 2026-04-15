import React, { useState, useEffect, useRef } from "react";

interface ToolEntry {
  name: string;
  desc: string;
  sample: string;
}

const tools: ToolEntry[] = [
  {
    name: "add_memory",
    desc: "Ingest text, extract entities and relations, store with embeddings.",
    sample: `{
  "entities_created": 2,
  "relations_created": 1,
  "entities": [
    { "name": "Alice", "type": "person" },
    { "name": "Acme Corp", "type": "org" }
  ],
  "relations": [
    { "from": "Alice", "to": "Acme Corp", "type": "works_at" }
  ]
}`,
  },
  {
    name: "search_memory",
    desc: "Hybrid vector + keyword search with RRF fusion.",
    sample: `{
  "results": [
    {
      "score": 0.847,
      "content": "Alice is a senior engineer at Acme Corp",
      "entities": ["Alice", "Acme Corp"]
    }
  ],
  "total": 1
}`,
  },
  {
    name: "expand_search",
    desc: "LLM rewrites query into semantic variants, merges results.",
    sample: `{
  "original_query": "Who works at Acme?",
  "expanded_queries": [
    "Acme Corp employees",
    "people affiliated with Acme"
  ],
  "results": [
    { "score": 0.91, "content": "Alice is a senior engineer at Acme Corp" },
    { "score": 0.78, "content": "Bob manages infrastructure at Acme" }
  ]
}`,
  },
  {
    name: "get_entity",
    desc: "Look up entity by name with type, summary, temporal bounds.",
    sample: `{
  "name": "Alice",
  "type": "person",
  "summary": "Senior engineer at Acme Corp",
  "valid_from": "2025-01-15T00:00:00Z",
  "relations": [
    { "to": "Acme Corp", "type": "works_at" },
    { "to": "Platform", "type": "member_of" }
  ]
}`,
  },
  {
    name: "snapshot",
    desc: "Point-in-time graph state for any timestamp.",
    sample: `{
  "timestamp": "2025-03-01T00:00:00Z",
  "entities": 12,
  "relations": 18,
  "sample": [
    { "name": "Alice", "type": "person", "active": true }
  ]
}`,
  },
  {
    name: "list_recent",
    desc: "N most recent memories ordered by creation time.",
    sample: `{
  "memories": [
    {
      "id": "mem_3f2a",
      "content": "Alice moved to the platform team",
      "created_at": "2025-04-10T14:30:00Z"
    }
  ]
}`,
  },
  {
    name: "list_agents",
    desc: "Which AI agents contributed to the graph.",
    sample: `{
  "agents": [
    { "id": "cursor-abc", "namespace": "work", "episodes": 42 },
    { "id": "claude-xyz", "namespace": "personal", "episodes": 17 }
  ]
}`,
  },
  {
    name: "list_namespaces",
    desc: "All namespaces with entity counts.",
    sample: `{
  "namespaces": [
    { "name": "work", "entities": 34, "relations": 56 },
    { "name": "personal", "entities": 12, "relations": 8 }
  ]
}`,
  },
  {
    name: "agent_activity",
    desc: "Audit a specific agent's recent contributions.",
    sample: `{
  "agent_id": "cursor-abc",
  "recent_episodes": [
    { "id": "ep_01", "memories": 3, "created_at": "2025-04-10T14:00:00Z" }
  ]
}`,
  },
  {
    name: "cross_namespace_search",
    desc: "Search across all namespaces globally.",
    sample: `{
  "results": [
    { "namespace": "work", "score": 0.85, "content": "Q4 roadmap approved" },
    { "namespace": "personal", "score": 0.72, "content": "Career goals for 2025" }
  ]
}`,
  },
];

function TypewriterOutput({ text, active }: { text: string; active: boolean }) {
  const [displayed, setDisplayed] = useState("");
  const [done, setDone] = useState(false);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    setDisplayed("");
    setDone(false);

    if (!active) return;

    let i = 0;
    const chars = text.split("");

    function tick() {
      if (i < chars.length) {
        const chunk = Math.min(3, chars.length - i);
        setDisplayed((prev) => prev + chars.slice(i, i + chunk).join(""));
        i += chunk;
        timeoutRef.current = setTimeout(tick, 12);
      } else {
        setDone(true);
      }
    }

    timeoutRef.current = setTimeout(tick, 300);

    return () => {
      if (timeoutRef.current) clearTimeout(timeoutRef.current);
    };
  }, [text, active]);

  return (
    <span className="terminal-output-json">
      {displayed}
      {!done && <span className="terminal-cursor" />}
    </span>
  );
}

export default function ToolTerminal() {
  const [selected, setSelected] = useState(0);
  const tool = tools[selected];

  return (
    <div className="terminal">
      <div className="terminal-chrome">
        <span className="terminal-dot" />
        <span className="terminal-dot" />
        <span className="terminal-dot" />
        <span className="terminal-title">context-keeper-mcp</span>
      </div>
      <div className="terminal-body">
        <div className="terminal-sidebar">
          {tools.map((t, i) => (
            <button
              key={t.name}
              className={`terminal-command ${i === selected ? "active" : ""}`}
              onClick={() => setSelected(i)}
            >
              {t.name}
            </button>
          ))}
        </div>
        <div className="terminal-output">
          <div className="terminal-output-prompt">
            $ {tool.name}
          </div>
          <div className="terminal-output-comment">
            # {tool.desc}
          </div>
          <div style={{ marginTop: "0.75rem" }}>
            <TypewriterOutput
              key={selected}
              text={tool.sample}
              active={true}
            />
          </div>
        </div>
      </div>
    </div>
  );
}
