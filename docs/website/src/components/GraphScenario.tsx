import React, { useState, useEffect } from "react";

// ── Scenario data ──────────────────────────────────────────────────
// Each step shows a memory being added and the graph state evolving.

interface Entity {
  id: string;
  label: string;
  type: string;
  x: number;
  y: number;
}

interface Relation {
  from: string;
  to: string;
  label: string;
}

interface ScenarioStep {
  memory: string;
  source: string;
  entities: Entity[];
  relations: Relation[];
  highlight?: string; // entity id to pulse
}

const steps: ScenarioStep[] = [
  {
    memory: '"Alice is a senior engineer at Acme Corp"',
    source: "chat",
    entities: [
      { id: "alice", label: "Alice", type: "person", x: 180, y: 100 },
      { id: "acme", label: "Acme Corp", type: "org", x: 420, y: 100 },
    ],
    relations: [{ from: "alice", to: "acme", label: "works_at" }],
    highlight: "alice",
  },
  {
    memory: '"Bob manages the infrastructure team at Acme"',
    source: "chat",
    entities: [
      { id: "alice", label: "Alice", type: "person", x: 140, y: 70 },
      { id: "acme", label: "Acme Corp", type: "org", x: 420, y: 70 },
      { id: "bob", label: "Bob", type: "person", x: 140, y: 190 },
      { id: "infra", label: "Infrastructure", type: "team", x: 420, y: 190 },
    ],
    relations: [
      { from: "alice", to: "acme", label: "works_at" },
      { from: "bob", to: "acme", label: "works_at" },
      { from: "bob", to: "infra", label: "manages" },
    ],
    highlight: "bob",
  },
  {
    memory: '"Alice moved to the platform team last week"',
    source: "standup",
    entities: [
      { id: "alice", label: "Alice", type: "person", x: 140, y: 70 },
      { id: "acme", label: "Acme Corp", type: "org", x: 420, y: 40 },
      { id: "bob", label: "Bob", type: "person", x: 140, y: 190 },
      { id: "infra", label: "Infrastructure", type: "team", x: 420, y: 190 },
      { id: "platform", label: "Platform", type: "team", x: 300, y: 130 },
    ],
    relations: [
      { from: "alice", to: "acme", label: "works_at" },
      { from: "alice", to: "platform", label: "member_of" },
      { from: "bob", to: "acme", label: "works_at" },
      { from: "bob", to: "infra", label: "manages" },
    ],
    highlight: "platform",
  },
];

// ── Entity colors by type ──────────────────────────────────────────
function typeColor(type: string): string {
  switch (type) {
    case "person": return "var(--ck-accent)";
    case "org":    return "#eab308";
    case "team":   return "#22d3ee";
    default:       return "var(--ck-accent)";
  }
}

// ── Main Component ─────────────────────────────────────────────────
export default function GraphScenario() {
  const [step, setStep] = useState(0);
  const [autoPlay, setAutoPlay] = useState(true);

  useEffect(() => {
    if (!autoPlay) return;
    const timer = setInterval(() => {
      setStep((s) => (s + 1) % steps.length);
    }, 4000);
    return () => clearInterval(timer);
  }, [autoPlay]);

  const current = steps[step];

  return (
    <div className="graph-scenario">
      {/* Memory input display */}
      <div className="graph-scenario-input">
        <div className="graph-scenario-source">{current.source}</div>
        <div className="graph-scenario-memory">{current.memory}</div>
      </div>

      {/* Graph visualization */}
      <div className="graph-scenario-viz">
        <svg
          viewBox="0 0 560 260"
          preserveAspectRatio="xMidYMid meet"
          className="graph-scenario-svg"
        >
          {/* Relations (edges) */}
          {current.relations.map((r, i) => {
            const from = current.entities.find((e) => e.id === r.from);
            const to = current.entities.find((e) => e.id === r.to);
            if (!from || !to) return null;
            const mx = (from.x + to.x) / 2;
            const my = (from.y + to.y) / 2 - 10;
            return (
              <g key={`${r.from}-${r.to}-${i}`} className="graph-edge-appear">
                <line
                  x1={from.x}
                  y1={from.y}
                  x2={to.x}
                  y2={to.y}
                  stroke="var(--ck-border)"
                  strokeWidth="1.5"
                  opacity="0.5"
                />
                <text
                  x={mx}
                  y={my}
                  textAnchor="middle"
                  className="graph-edge-label"
                >
                  {r.label}
                </text>
              </g>
            );
          })}

          {/* Entities (nodes) */}
          {current.entities.map((e) => {
            const isHighlight = e.id === current.highlight;
            const color = typeColor(e.type);
            return (
              <g key={e.id} className={`graph-node-appear ${isHighlight ? "graph-node-pulse" : ""}`}>
                <circle
                  cx={e.x}
                  cy={e.y}
                  r={isHighlight ? 30 : 26}
                  fill="var(--ck-surface)"
                  stroke={color}
                  strokeWidth={isHighlight ? 2.5 : 1.5}
                  opacity={isHighlight ? 1 : 0.85}
                />
                {isHighlight && (
                  <circle
                    cx={e.x}
                    cy={e.y}
                    r={36}
                    fill="none"
                    stroke={color}
                    strokeWidth="1"
                    opacity="0.3"
                    className="graph-pulse-ring"
                  />
                )}
                <text
                  x={e.x}
                  y={e.y + 1}
                  textAnchor="middle"
                  dominantBaseline="middle"
                  className="graph-node-label"
                  fill={color}
                >
                  {e.label}
                </text>
                <text
                  x={e.x}
                  y={e.y + 46}
                  textAnchor="middle"
                  className="graph-node-type"
                >
                  {e.type}
                </text>
              </g>
            );
          })}
        </svg>
      </div>

      {/* Step indicator */}
      <div className="graph-scenario-steps">
        {steps.map((_, i) => (
          <button
            key={i}
            className={`graph-step-dot ${i === step ? "active" : ""}`}
            onClick={() => { setStep(i); setAutoPlay(false); }}
            aria-label={`Step ${i + 1}`}
          />
        ))}
      </div>
    </div>
  );
}
