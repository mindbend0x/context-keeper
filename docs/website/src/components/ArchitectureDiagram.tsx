import React, { useState } from "react";

/*
  Hub-and-spoke layout: core at center, binaries above, implementations below.
  All edges radiate from/to core, making the architecture immediately clear.

  SVG viewBox: 640 x 400
*/

const nodes = {
  cli:     { x: 110, y: 30,  w: 150, h: 72, label: "cli",     sub: "Developer CLI",              icon: "terminal", layer: "binary" },
  mcp:     { x: 380, y: 30,  w: 150, h: 72, label: "mcp",     sub: "MCP Server",                 icon: "plug",     layer: "binary" },
  core:    { x: 235, y: 175, w: 170, h: 80, label: "core",    sub: "Pipeline · Search · Traits",  icon: "gear",     layer: "core" },
  rig:     { x: 50,  y: 320, w: 160, h: 72, label: "rig",     sub: "LLM Integrations",           icon: "brain",    layer: "impl" },
  surreal: { x: 430, y: 320, w: 160, h: 72, label: "surreal", sub: "SurrealDB · Storage",        icon: "database", layer: "impl" },
} as const;

type NodeId = keyof typeof nodes;

interface Edge {
  from: NodeId;
  to: NodeId;
  type: "depends" | "implements" | "wires";
  label?: string;
}

const edges: Edge[] = [
  { from: "cli", to: "core",    type: "depends",    label: "uses" },
  { from: "mcp", to: "core",    type: "depends",    label: "uses" },
  { from: "rig", to: "core",    type: "implements", label: "implements" },
  { from: "surreal", to: "core", type: "implements", label: "implements" },
  { from: "cli", to: "rig",     type: "wires" },
  { from: "cli", to: "surreal", type: "wires" },
  { from: "mcp", to: "rig",     type: "wires" },
  { from: "mcp", to: "surreal", type: "wires" },
];

function NodeIcon({ type, x, y }: { type: string; x: number; y: number }) {
  const size = 16;

  switch (type) {
    case "terminal":
      return (
        <g transform={`translate(${x},${y})`}>
          <rect width={size} height={size} rx="2" fill="none" stroke="var(--ck-accent)" strokeWidth="1.5" opacity={0.9} />
          <path d="M4 5l4 3-4 3M9 11h3" stroke="var(--ck-accent)" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" opacity={0.9} />
        </g>
      );
    case "plug":
      return (
        <g transform={`translate(${x},${y})`}>
          <circle cx="8" cy="8" r="6" fill="none" stroke="var(--ck-accent)" strokeWidth="1.5" opacity={0.9} />
          <path d="M6 6v4M10 6v4" stroke="var(--ck-accent)" strokeWidth="1.5" strokeLinecap="round" opacity={0.9} />
        </g>
      );
    case "brain":
      return (
        <g transform={`translate(${x},${y})`}>
          <circle cx="8" cy="8" r="6" fill="none" stroke="var(--ck-accent)" strokeWidth="1.5" opacity={0.9} />
          <path d="M5 8h6" stroke="var(--ck-accent)" strokeWidth="1" opacity={0.6} />
          <path d="M8 5v6" stroke="var(--ck-accent)" strokeWidth="1" opacity={0.6} />
        </g>
      );
    case "gear":
      return (
        <g transform={`translate(${x},${y})`}>
          <circle cx="8" cy="8" r="3" fill="none" stroke="var(--ck-accent)" strokeWidth="1.5" opacity={0.9} />
          <circle cx="8" cy="8" r="6" fill="none" stroke="var(--ck-accent)" strokeWidth="1.5" strokeDasharray="3 2" opacity={0.6} />
        </g>
      );
    case "database":
      return (
        <g transform={`translate(${x},${y})`}>
          <ellipse cx="8" cy="5" rx="5" ry="2.5" fill="none" stroke="var(--ck-accent)" strokeWidth="1.5" opacity={0.9} />
          <path d="M3 5v6c0 1.4 2.2 2.5 5 2.5s5-1.1 5-2.5V5" fill="none" stroke="var(--ck-accent)" strokeWidth="1.5" opacity={0.9} />
        </g>
      );
    default:
      return null;
  }
}

function edgePath(from: NodeId, to: NodeId): string {
  const f = nodes[from];
  const t = nodes[to];
  const fx = f.x + f.w / 2;
  const tx = t.x + t.w / 2;

  const fromAbove = f.y < t.y;
  const fy = fromAbove ? f.y + f.h : f.y;
  const ty = fromAbove ? t.y : t.y + t.h;

  const mid = (fy + ty) / 2;
  return `M ${fx} ${fy} C ${fx} ${mid}, ${tx} ${mid}, ${tx} ${ty}`;
}

function edgeStyle(type: Edge["type"], active: boolean, dimmed: boolean) {
  const base = {
    depends: {
      stroke: active ? "var(--ck-accent)" : "var(--ck-border)",
      strokeWidth: active ? 2.5 : 1.5,
      strokeDasharray: "none",
      opacity: dimmed ? 0.1 : active ? 1 : 0.4,
    },
    implements: {
      stroke: active ? "var(--ck-entity-org)" : "var(--ck-border)",
      strokeWidth: active ? 2.5 : 2,
      strokeDasharray: "none",
      opacity: dimmed ? 0.1 : active ? 1 : 0.5,
    },
    wires: {
      stroke: active ? "var(--ck-text-muted)" : "var(--ck-border)",
      strokeWidth: 1,
      strokeDasharray: "5 4",
      opacity: dimmed ? 0.05 : active ? 0.6 : 0.2,
    },
  };
  return base[type];
}

export default function ArchitectureDiagram() {
  const [hovered, setHovered] = useState<NodeId | null>(null);

  function isEdgeActive(e: Edge) {
    if (!hovered) return false;
    return e.from === hovered || e.to === hovered;
  }

  return (
    <div className="arch-diagram-container">
      <svg
        className="arch-diagram-svg"
        viewBox="0 0 640 420"
        preserveAspectRatio="xMidYMid meet"
        role="img"
        aria-label="Context Keeper architecture: core at center, cli and mcp above, rig and surreal below"
      >
        <defs>
          <marker id="arr-depends" viewBox="0 0 10 8" refX="9" refY="4" markerWidth="7" markerHeight="5" orient="auto-start-reverse">
            <path d="M0 0 L10 4 L0 8 Z" fill="var(--ck-accent)" />
          </marker>
          <marker id="arr-depends-dim" viewBox="0 0 10 8" refX="9" refY="4" markerWidth="7" markerHeight="5" orient="auto-start-reverse">
            <path d="M0 0 L10 4 L0 8 Z" fill="var(--ck-border)" opacity="0.5" />
          </marker>
          <marker id="arr-impl" viewBox="0 0 10 8" refX="9" refY="4" markerWidth="7" markerHeight="5" orient="auto-start-reverse">
            <path d="M0 0 L10 4 L0 8 Z" fill="var(--ck-entity-org)" />
          </marker>
          <marker id="arr-impl-dim" viewBox="0 0 10 8" refX="9" refY="4" markerWidth="7" markerHeight="5" orient="auto-start-reverse">
            <path d="M0 0 L10 4 L0 8 Z" fill="var(--ck-border)" opacity="0.4" />
          </marker>
        </defs>

        {/* Layer labels */}
        <text x="16" y="68" className="arch-layer-label">BINARIES</text>
        <text x="16" y="230" className="arch-layer-label">CORE</text>
        <text x="16" y="360" className="arch-layer-label">PROVIDERS</text>

        {/* Edges — wires first (behind), then structural */}
        {edges
          .slice()
          .sort((a, b) => {
            const order = { wires: 0, depends: 1, implements: 2 };
            return order[a.type] - order[b.type];
          })
          .map((e, i) => {
            const active = isEdgeActive(e);
            const dimmed = !!hovered && !active;
            const style = edgeStyle(e.type, active, dimmed);
            const marker =
              e.type === "implements"
                ? active ? "url(#arr-impl)" : "url(#arr-impl-dim)"
                : e.type === "depends"
                  ? active ? "url(#arr-depends)" : "url(#arr-depends-dim)"
                  : undefined;

            return (
              <path
                key={`${e.from}-${e.to}-${i}`}
                d={edgePath(e.from, e.to)}
                fill="none"
                stroke={style.stroke}
                strokeWidth={style.strokeWidth}
                strokeDasharray={style.strokeDasharray}
                opacity={style.opacity}
                markerEnd={marker}
                className="arch-edge"
              />
            );
          })}

        {/* Edge labels for primary connections (only when hovered) */}
        {hovered && edges
          .filter((e) => (e.from === hovered || e.to === hovered) && e.label)
          .map((e, i) => {
            const f = nodes[e.from];
            const t = nodes[e.to];
            const mx = (f.x + f.w / 2 + t.x + t.w / 2) / 2;
            const fromAbove = f.y < t.y;
            const fy = fromAbove ? f.y + f.h : f.y;
            const ty = fromAbove ? t.y : t.y + t.h;
            const my = (fy + ty) / 2 - 6;
            return (
              <text
                key={`label-${i}`}
                x={mx}
                y={my}
                textAnchor="middle"
                style={{
                  fontFamily: "var(--ifm-font-family-monospace)",
                  fontSize: "9px",
                  fill: e.type === "implements" ? "var(--ck-entity-org)" : "var(--ck-accent)",
                  opacity: 0.7,
                }}
              >
                {e.label}
              </text>
            );
          })}

        {/* Nodes */}
        {(Object.entries(nodes) as [NodeId, (typeof nodes)[NodeId]][]).map(([id, n]) => {
          const isCore = id === "core";
          const isHovered = hovered === id;
          const dimmed = !!hovered && !isHovered && !edges.some(
            (e) => (e.from === hovered && e.to === id) || (e.to === hovered && e.from === id)
          );

          return (
            <g
              key={id}
              onMouseEnter={() => setHovered(id)}
              onMouseLeave={() => setHovered(null)}
              style={{ cursor: "default" }}
            >
              {isHovered && (
                <rect
                  x={n.x - 4} y={n.y - 4}
                  width={n.w + 8} height={n.h + 8}
                  rx="14"
                  fill={isCore ? "var(--ck-accent)" : "var(--ck-accent)"}
                  opacity="0.06"
                />
              )}
              <rect
                x={n.x} y={n.y}
                width={n.w} height={n.h}
                rx="10"
                fill="var(--ck-surface)"
                stroke={isCore ? "var(--ck-accent)" : "var(--ck-border)"}
                strokeWidth={isCore ? 2 : 1.5}
                opacity={dimmed ? 0.35 : 1}
                className="arch-node-rect"
              />
              <NodeIcon type={n.icon} x={n.x + 14} y={n.y + 14} />
              <text
                x={n.x + 38} y={n.y + 28}
                className="arch-node-label"
                opacity={dimmed ? 0.35 : 1}
              >
                {n.label}
              </text>
              <text
                x={n.x + n.w / 2} y={n.y + 52}
                textAnchor="middle"
                className="arch-node-sub"
                opacity={dimmed ? 0.25 : 0.7}
              >
                {n.sub}
              </text>
            </g>
          );
        })}
      </svg>

      <div className="arch-legend">
        <div className="arch-legend-item">
          <svg width="28" height="8">
            <line x1="0" y1="4" x2="28" y2="4" stroke="var(--ck-accent)" strokeWidth="2" />
          </svg>
          <span>Depends on</span>
        </div>
        <div className="arch-legend-item">
          <svg width="28" height="8">
            <line x1="0" y1="4" x2="28" y2="4" stroke="var(--ck-entity-org)" strokeWidth="2" />
          </svg>
          <span>Implements traits</span>
        </div>
        <div className="arch-legend-item">
          <svg width="28" height="8">
            <line x1="0" y1="4" x2="28" y2="4" stroke="var(--ck-text-muted)" strokeWidth="1" strokeDasharray="5 4" />
          </svg>
          <span>Wires concrete types</span>
        </div>
      </div>
    </div>
  );
}
