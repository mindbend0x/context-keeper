import React, { useEffect, useState } from "react";

interface GraphNode {
  id: string;
  label: string;
  type: "person" | "org" | "team" | "concept";
  x: number;
  y: number;
  delay: number;
}

interface GraphEdge {
  from: string;
  to: string;
  delay: number;
}

const nodes: GraphNode[] = [
  { id: "alice", label: "Alice", type: "person", x: 0.08, y: 0.2, delay: 200 },
  { id: "bob", label: "Bob", type: "person", x: 0.88, y: 0.15, delay: 600 },
  { id: "acme", label: "Acme Corp", type: "org", x: 0.72, y: 0.65, delay: 400 },
  { id: "platform", label: "Platform", type: "team", x: 0.22, y: 0.72, delay: 800 },
  { id: "infra", label: "Infra", type: "team", x: 0.5, y: 0.12, delay: 1000 },
  { id: "api", label: "API Design", type: "concept", x: 0.38, y: 0.42, delay: 1200 },
  { id: "deploy", label: "Deploy", type: "concept", x: 0.06, y: 0.5, delay: 1400 },
  { id: "review", label: "Review", type: "concept", x: 0.62, y: 0.38, delay: 1600 },
  { id: "carol", label: "Carol", type: "person", x: 0.92, y: 0.55, delay: 1800 },
  { id: "dave", label: "Dave", type: "person", x: 0.15, y: 0.88, delay: 2000 },
  { id: "security", label: "Security", type: "team", x: 0.78, y: 0.85, delay: 2200 },
  { id: "roadmap", label: "Roadmap", type: "concept", x: 0.52, y: 0.78, delay: 2400 },
  { id: "sprint", label: "Sprint", type: "concept", x: 0.35, y: 0.15, delay: 2600 },
  { id: "eve", label: "Eve", type: "person", x: 0.95, y: 0.3, delay: 2800 },
];

const edges: GraphEdge[] = [
  { from: "alice", to: "acme", delay: 500 },
  { from: "bob", to: "acme", delay: 700 },
  { from: "alice", to: "platform", delay: 900 },
  { from: "bob", to: "infra", delay: 1100 },
  { from: "platform", to: "api", delay: 1300 },
  { from: "infra", to: "deploy", delay: 1500 },
  { from: "carol", to: "security", delay: 1900 },
  { from: "dave", to: "platform", delay: 2100 },
  { from: "review", to: "api", delay: 1700 },
  { from: "security", to: "acme", delay: 2300 },
  { from: "roadmap", to: "platform", delay: 2500 },
  { from: "sprint", to: "infra", delay: 2700 },
  { from: "eve", to: "review", delay: 2900 },
  { from: "roadmap", to: "security", delay: 3100 },
];

function typeColor(type: string): string {
  switch (type) {
    case "person": return "var(--ck-entity-person)";
    case "org": return "var(--ck-entity-org)";
    case "team": return "var(--ck-entity-team)";
    case "concept": return "var(--ck-entity-concept)";
    default: return "var(--ck-accent)";
  }
}

function Particle({ x1, y1, x2, y2, delay }: { x1: number; y1: number; x2: number; y2: number; delay: number }) {
  const dur = 3 + Math.random() * 2;
  return (
    <circle r="1.5" fill="var(--ck-particle-color)" opacity="0">
      <animateMotion
        path={`M${x1},${y1} L${x2},${y2}`}
        dur={`${dur}s`}
        begin={`${delay / 1000 + 2}s`}
        repeatCount="indefinite"
      />
      <animate
        attributeName="opacity"
        values="0;0.6;0.6;0"
        keyTimes="0;0.1;0.9;1"
        dur={`${dur}s`}
        begin={`${delay / 1000 + 2}s`}
        repeatCount="indefinite"
      />
    </circle>
  );
}

export default function HeroGraph() {
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setMounted(true);
  }, []);

  if (!mounted) return <div className="hero-graph-bg" />;

  const vw = 1000;
  const vh = 500;

  const nodeMap = new Map(nodes.map((n) => [n.id, n]));

  return (
    <div className="hero-graph-bg">
      <svg viewBox={`0 0 ${vw} ${vh}`} preserveAspectRatio="xMidYMid slice">
        {edges.map((e, i) => {
          const from = nodeMap.get(e.from);
          const to = nodeMap.get(e.to);
          if (!from || !to) return null;
          const x1 = from.x * vw;
          const y1 = from.y * vh;
          const x2 = to.x * vw;
          const y2 = to.y * vh;
          return (
            <g key={`edge-${i}`}>
              <line
                x1={x1} y1={y1} x2={x2} y2={y2}
                className="hero-graph-edge"
                stroke="var(--ck-edge-color)"
                strokeWidth="1"
                style={{ animationDelay: `${e.delay}ms` }}
              />
              <Particle x1={x1} y1={y1} x2={x2} y2={y2} delay={e.delay} />
            </g>
          );
        })}

        {nodes.map((n) => {
          const cx = n.x * vw;
          const cy = n.y * vh;
          const color = typeColor(n.type);
          return (
            <g key={n.id}>
              <circle
                cx={cx} cy={cy} r="18"
                className="hero-graph-glow"
                fill={color}
                opacity="0"
                style={{ animationDelay: `${n.delay + 400}ms` }}
              />
              <circle
                cx={cx} cy={cy} r="4"
                className="hero-graph-node"
                fill={color}
                style={{ animationDelay: `${n.delay}ms` }}
              />
              <text
                x={cx} y={cy + 14}
                textAnchor="middle"
                className="hero-graph-label"
                style={{ animationDelay: `${n.delay + 200}ms` }}
              >
                {n.label}
              </text>
            </g>
          );
        })}
      </svg>
    </div>
  );
}
