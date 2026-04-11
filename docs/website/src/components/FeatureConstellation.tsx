import React, { useState } from "react";
import {
  TemporalGraphIcon,
  HybridSearchIcon,
  McpNativeIcon,
  TraitArchIcon,
  SurrealDbIcon,
  ShipAnywhereIcon,
} from "./Icons";

interface Feature {
  id: string;
  title: string;
  desc: string;
  Icon: React.ComponentType<{ size?: number; className?: string }>;
  color: string;
}

const features: Feature[] = [
  {
    id: "temporal",
    title: "Temporal Knowledge Graph",
    desc: "Every entity and relation carries valid_from / valid_until timestamps. Point-in-time snapshots let you query the graph at any moment.",
    Icon: TemporalGraphIcon,
    color: "var(--ck-entity-person)",
  },
  {
    id: "search",
    title: "Hybrid Search + RRF",
    desc: "HNSW vector similarity and BM25 keyword search, fused with Reciprocal Rank Fusion (K=60). LLM-powered query expansion.",
    Icon: HybridSearchIcon,
    color: "var(--ck-entity-team)",
  },
  {
    id: "mcp",
    title: "MCP Native",
    desc: "10 tools, browsable entity resources, and 3 prompt templates. Works with Claude Desktop, Cursor, and any MCP-compatible client.",
    Icon: McpNativeIcon,
    color: "var(--ck-entity-org)",
  },
  {
    id: "traits",
    title: "Trait-Based Architecture",
    desc: "Core defines pure traits for embedders, extractors, and query rewriters. Swap providers without touching the pipeline.",
    Icon: TraitArchIcon,
    color: "var(--ck-highlight)",
  },
  {
    id: "surreal",
    title: "SurrealDB All-in-One",
    desc: "One database for documents, graph edges, vector indexes, and full-text search. No middleware glue.",
    Icon: SurrealDbIcon,
    color: "var(--ck-entity-person)",
  },
  {
    id: "ship",
    title: "Ship Anywhere",
    desc: "Run as an MCP server (stdio or HTTP), a CLI tool, or a Docker container. RocksDB persistence by default.",
    Icon: ShipAnywhereIcon,
    color: "var(--ck-entity-team)",
  },
];

const stats = [
  { value: "5", label: "Rust crates" },
  { value: "10", label: "MCP tools" },
  { value: "35+", label: "DB operations" },
  { value: "0", label: "API keys to test" },
];

export default function FeatureConstellation() {
  const [hovered, setHovered] = useState<string | null>(null);

  const cx = 400;
  const cy = 220;
  const radius = 170;
  const centerR = 36;
  const nodeR = 28;

  const positioned = features.map((f, i) => {
    const angle = (i / features.length) * Math.PI * 2 - Math.PI / 2;
    return {
      ...f,
      x: cx + Math.cos(angle) * radius,
      y: cy + Math.sin(angle) * radius,
    };
  });

  return (
    <div className="constellation">
      <div className="constellation-container">
        <div className="constellation-svg-wrapper">
          <svg
            viewBox="0 0 800 440"
            className="constellation-svg"
            preserveAspectRatio="xMidYMid meet"
          >
            {positioned.map((f) => (
              <line
                key={`edge-${f.id}`}
                x1={cx}
                y1={cy}
                x2={f.x}
                y2={f.y}
                className="constellation-edge"
                style={
                  hovered === f.id
                    ? { stroke: f.color, strokeWidth: 2, opacity: 1 }
                    : hovered
                      ? { opacity: 0.3 }
                      : {}
                }
              />
            ))}

            {positioned.map((f, i) => {
              const pathId = `cpath-${i}`;
              return (
                <g key={`particle-${i}`}>
                  <path
                    id={pathId}
                    d={`M${cx},${cy} L${f.x},${f.y}`}
                    fill="none"
                    stroke="none"
                  />
                  <circle r="2" fill={f.color} opacity="0">
                    <animateMotion dur="3s" begin={`${i * 0.5}s`} repeatCount="indefinite">
                      <mpath href={`#${pathId}`} />
                    </animateMotion>
                    <animate
                      attributeName="opacity"
                      values="0;0.5;0.5;0"
                      keyTimes="0;0.15;0.85;1"
                      dur="3s"
                      begin={`${i * 0.5}s`}
                      repeatCount="indefinite"
                    />
                  </circle>
                </g>
              );
            })}

            <circle cx={cx} cy={cy} r={centerR + 8} fill="var(--ck-node-glow)" opacity="0.4" />
            <circle cx={cx} cy={cy} r={centerR} fill="var(--ck-surface)" stroke="var(--ck-accent)" strokeWidth="2" />
            <text x={cx} y={cy - 3} textAnchor="middle" dominantBaseline="middle" className="constellation-center-label">
              CTX.K
            </text>
            <text x={cx} y={cy + 14} textAnchor="middle" className="constellation-center-sub">
              core
            </text>

            {positioned.map((f) => {
              const isHovered = hovered === f.id;
              return (
                <g
                  key={f.id}
                  className="constellation-feature"
                  onMouseEnter={() => setHovered(f.id)}
                  onMouseLeave={() => setHovered(null)}
                  style={hovered && !isHovered ? { opacity: 0.5 } : {}}
                >
                  <circle
                    cx={f.x} cy={f.y} r={isHovered ? nodeR + 4 : nodeR}
                    fill="var(--ck-surface)"
                    stroke={f.color}
                    strokeWidth={isHovered ? 2.5 : 1.5}
                    className="constellation-node-circle"
                  />
                  <foreignObject
                    x={f.x - 12} y={f.y - 12}
                    width="24" height="24"
                  >
                    <div
                      xmlns="http://www.w3.org/1999/xhtml"
                      style={{
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                        width: 24,
                        height: 24,
                        color: f.color,
                      }}
                    >
                      <f.Icon size={20} />
                    </div>
                  </foreignObject>
                  <text
                    x={f.x}
                    y={f.y + nodeR + 16}
                    textAnchor="middle"
                    className="constellation-feature-title"
                  >
                    {f.title}
                  </text>
                  {isHovered && (
                    <foreignObject
                      x={f.x - 100} y={f.y + nodeR + 26}
                      width="200" height="60"
                    >
                      <div
                        xmlns="http://www.w3.org/1999/xhtml"
                        style={{
                          fontSize: "10px",
                          color: "var(--ck-text-muted)",
                          textAlign: "center",
                          lineHeight: 1.35,
                          fontFamily: "var(--ifm-font-family-base)",
                        }}
                      >
                        {f.desc}
                      </div>
                    </foreignObject>
                  )}
                </g>
              );
            })}
          </svg>
        </div>

        <div className="constellation-grid-fallback">
          {features.map((f) => (
            <div key={f.id} className="constellation-grid-card">
              <div className="constellation-grid-icon" style={{ color: f.color }}>
                <f.Icon size={22} />
              </div>
              <div className="constellation-grid-content">
                <h3>{f.title}</h3>
                <p>{f.desc}</p>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="constellation-stats">
        {stats.map((s) => (
          <div className="constellation-stat" key={s.label}>
            <div className="constellation-stat-value">{s.value}</div>
            <div className="constellation-stat-label">{s.label}</div>
          </div>
        ))}
      </div>
    </div>
  );
}
