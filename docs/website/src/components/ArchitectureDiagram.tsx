import React, { useState } from "react";

// ── Node positions ─────────────────────────────────────────────────
// Layout: Top row (binaries) → Bottom row (foundation)
// SVG viewBox is 640x380

const nodes = {
  cli:     { x: 100, y: 40,  w: 140, h: 72, label: "cli",     sub: "Developer CLI",        icon: "terminal" },
  mcp:     { x: 400, y: 40,  w: 140, h: 72, label: "mcp",     sub: "MCP Server",           icon: "plug" },
  rig:     { x: 40,  y: 260, w: 150, h: 72, label: "rig",     sub: "LLM Integrations",     icon: "brain" },
  core:    { x: 245, y: 260, w: 150, h: 72, label: "core",    sub: "Pipeline · Search · Traits", icon: "gear" },
  surreal: { x: 450, y: 260, w: 150, h: 72, label: "surreal", sub: "SurrealDB · Storage",  icon: "database" },
} as const;

type NodeId = keyof typeof nodes;

// ── Edges (from → to, following actual Cargo.toml deps) ────────────
const edges: { from: NodeId; to: NodeId; style: "orchestrates" | "implements" }[] = [
  // Binaries orchestrate all three foundation crates
  { from: "cli", to: "core",    style: "orchestrates" },
  { from: "cli", to: "rig",     style: "orchestrates" },
  { from: "cli", to: "surreal", style: "orchestrates" },
  { from: "mcp", to: "core",    style: "orchestrates" },
  { from: "mcp", to: "rig",     style: "orchestrates" },
  { from: "mcp", to: "surreal", style: "orchestrates" },
  // Implementation crates depend on core
  { from: "rig",     to: "core", style: "implements" },
  { from: "surreal", to: "core", style: "implements" },
];

// ── Animated data flow paths ───────────────────────────────────────
const flowPaths = [
  {
    id: "ingest",
    label: "Ingestion",
    color: "var(--ck-accent)",
    // mcp top → core center → surreal center
    d: `M ${nodes.mcp.x + nodes.mcp.w / 2} ${nodes.mcp.y + nodes.mcp.h}
        L ${nodes.core.x + nodes.core.w / 2} ${nodes.core.y}
        L ${nodes.surreal.x + nodes.surreal.w / 2} ${nodes.surreal.y}`,
  },
  {
    id: "retrieve",
    label: "Retrieval",
    color: "var(--ck-accent-400, #fb923c)",
    // cli top → core center → rig center (for LLM query rewriting)
    d: `M ${nodes.cli.x + nodes.cli.w / 2} ${nodes.cli.y + nodes.cli.h}
        L ${nodes.core.x + nodes.core.w / 2} ${nodes.core.y}
        L ${nodes.rig.x + nodes.rig.w / 2} ${nodes.rig.y}`,
  },
];

// ── Icon paths (simple inline SVGs) ────────────────────────────────
function NodeIcon({ type, x, y }: { type: string; x: number; y: number }) {
  const size = 16;
  const props = { x, y, width: size, height: size, fill: "var(--ck-accent)", opacity: 0.9 };

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
          <path d="M8 4c-2 0-3 2-3 4s1 4 3 4 3-2 3-4-1-4-3-4z" fill="none" stroke="var(--ck-accent)" strokeWidth="1" opacity={0.7} />
          <path d="M5 8h6" stroke="var(--ck-accent)" strokeWidth="1" opacity={0.5} />
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
          <path d="M3 8c0 1.4 2.2 2.5 5 2.5s5-1.1 5-2.5" fill="none" stroke="var(--ck-accent)" strokeWidth="1" opacity={0.5} />
        </g>
      );
    default:
      return null;
  }
}

// ── Edge path calculator ───────────────────────────────────────────
function edgePath(from: NodeId, to: NodeId): string {
  const f = nodes[from];
  const t = nodes[to];
  const fx = f.x + f.w / 2;
  const fy = f.y + f.h;
  const tx = t.x + t.w / 2;
  const ty = t.y;

  // Bezier curve for smoother edges
  const cy1 = fy + (ty - fy) * 0.4;
  const cy2 = fy + (ty - fy) * 0.6;
  return `M ${fx} ${fy} C ${fx} ${cy1}, ${tx} ${cy2}, ${tx} ${ty}`;
}

// ── Main Component ─────────────────────────────────────────────────
export default function ArchitectureDiagram() {
  const [hoveredNode, setHoveredNode] = useState<NodeId | null>(null);

  function isEdgeActive(e: typeof edges[number]) {
    if (!hoveredNode) return false;
    return e.from === hoveredNode || e.to === hoveredNode;
  }

  return (
    <div className="arch-diagram-container">
      <svg
        className="arch-diagram-svg"
        viewBox="0 0 640 380"
        preserveAspectRatio="xMidYMid meet"
        role="img"
        aria-label="Context Keeper architecture diagram showing five Rust crates and their dependencies"
      >
        <defs>
          {/* Arrowhead markers */}
          <marker id="arrow-default" viewBox="0 0 10 8" refX="9" refY="4" markerWidth="8" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 4 L0 8 Z" fill="var(--ck-border)" opacity="0.5" />
          </marker>
          <marker id="arrow-active" viewBox="0 0 10 8" refX="9" refY="4" markerWidth="8" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 4 L0 8 Z" fill="var(--ck-accent)" />
          </marker>
          <marker id="arrow-implements" viewBox="0 0 10 8" refX="9" refY="4" markerWidth="8" markerHeight="6" orient="auto-start-reverse">
            <path d="M0 0 L10 4 L0 8 Z" fill="var(--ck-accent)" opacity="0.7" />
          </marker>
        </defs>

        {/* ── Layer labels ──────────────────────────────────── */}
        <text x="10" y="78" className="arch-layer-label">BINARIES</text>
        <text x="10" y="298" className="arch-layer-label">FOUNDATION</text>

        {/* ── Edges ─────────────────────────────────────────── */}
        {edges.map((e, i) => {
          const active = isEdgeActive(e);
          return (
            <path
              key={i}
              d={edgePath(e.from, e.to)}
              fill="none"
              stroke={active ? "var(--ck-accent)" : "var(--ck-border)"}
              strokeWidth={e.style === "implements" ? 2.5 : 1.5}
              strokeDasharray={e.style === "orchestrates" ? "6 4" : "none"}
              opacity={hoveredNode && !active ? 0.15 : (active ? 1 : 0.4)}
              markerEnd={
                active
                  ? "url(#arrow-active)"
                  : e.style === "implements"
                  ? "url(#arrow-implements)"
                  : "url(#arrow-default)"
              }
              className="arch-edge"
            />
          );
        })}

        {/* ── Animated flow paths ───────────────────────────── */}
        {flowPaths.map((fp) => (
          <g key={fp.id}>
            {/* Faint background path */}
            <path d={fp.d} fill="none" stroke={fp.color} strokeWidth="2" opacity="0.08" />
            {/* Animated pulse */}
            <path
              d={fp.d}
              fill="none"
              stroke={fp.color}
              strokeWidth="2.5"
              strokeDasharray="12 28"
              opacity="0.4"
              className={`arch-flow-pulse arch-flow-${fp.id}`}
            />
          </g>
        ))}

        {/* ── Nodes ─────────────────────────────────────────── */}
        {(Object.entries(nodes) as [NodeId, typeof nodes[NodeId]][]).map(([id, n]) => {
          const isCore = id === "core";
          const isHovered = hoveredNode === id;
          return (
            <g
              key={id}
              className={`arch-node-group ${isHovered ? "arch-node-hovered" : ""}`}
              onMouseEnter={() => setHoveredNode(id)}
              onMouseLeave={() => setHoveredNode(null)}
              style={{ cursor: "default" }}
            >
              {/* Glow effect on hover */}
              {isHovered && (
                <rect
                  x={n.x - 4}
                  y={n.y - 4}
                  width={n.w + 8}
                  height={n.h + 8}
                  rx="14"
                  fill="var(--ck-accent)"
                  opacity="0.08"
                />
              )}
              {/* Node background */}
              <rect
                x={n.x}
                y={n.y}
                width={n.w}
                height={n.h}
                rx="10"
                fill="var(--ck-surface)"
                stroke={isCore ? "var(--ck-accent)" : "var(--ck-border)"}
                strokeWidth={isCore ? 2 : 1.5}
                opacity={hoveredNode && !isHovered ? 0.5 : 1}
                className="arch-node-rect"
              />
              {/* Icon */}
              <NodeIcon type={n.icon} x={n.x + 12} y={n.y + 12} />
              {/* Label */}
              <text
                x={n.x + 36}
                y={n.y + 26}
                className="arch-node-label"
                opacity={hoveredNode && !isHovered ? 0.5 : 1}
              >
                {n.label}
              </text>
              {/* Subtitle */}
              <text
                x={n.x + n.w / 2}
                y={n.y + 50}
                textAnchor="middle"
                className="arch-node-sub"
                opacity={hoveredNode && !isHovered ? 0.4 : 0.7}
              >
                {n.sub}
              </text>
            </g>
          );
        })}
      </svg>

      {/* ── Legend ───────────────────────────────────────────── */}
      <div className="arch-legend">
        <div className="arch-legend-item">
          <svg width="32" height="8"><line x1="0" y1="4" x2="32" y2="4" stroke="var(--ck-accent)" strokeWidth="2.5" strokeDasharray="6 4" /></svg>
          <span>Orchestrates</span>
        </div>
        <div className="arch-legend-item">
          <svg width="32" height="8"><line x1="0" y1="4" x2="32" y2="4" stroke="var(--ck-accent)" strokeWidth="2.5" /></svg>
          <span>Implements traits</span>
        </div>
        <div className="arch-legend-item">
          <svg width="32" height="8">
            <line x1="0" y1="4" x2="32" y2="4" stroke="var(--ck-accent)" strokeWidth="2.5" strokeDasharray="12 28" className="arch-flow-pulse arch-flow-legend" />
          </svg>
          <span>Data flow</span>
        </div>
      </div>
    </div>
  );
}
