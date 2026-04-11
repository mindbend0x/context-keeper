import React, { useState } from "react";

interface PipelineNode {
  num: string;
  title: string;
  desc: string;
}

const ingestSteps: PipelineNode[] = [
  { num: "1", title: "Ingest", desc: "Agent sends text via add_memory. Source tagged by type." },
  { num: "2", title: "Extract", desc: "LLM extracts entities and relationships from text." },
  { num: "3", title: "Embed", desc: "Vector embeddings + BM25 index for hybrid search." },
  { num: "4", title: "Store", desc: "Entities upserted with temporal bounds. Graph evolves." },
];

const retrieveSteps: PipelineNode[] = [
  { num: "A", title: "Search", desc: "Hybrid vector + keyword search fused with RRF." },
  { num: "B", title: "Expand", desc: "LLM rewrites query into semantic variants." },
  { num: "C", title: "Traverse", desc: "Follow entity relationships through the graph." },
  { num: "D", title: "Snapshot", desc: "Query the graph at any point in time." },
];

function PipelineRow({
  steps,
  yOffset,
  color,
  hoveredIdx,
  onHover,
}: {
  steps: PipelineNode[];
  yOffset: number;
  color: string;
  hoveredIdx: number | null;
  onHover: (idx: number | null) => void;
}) {
  const nodeR = 26;
  const startX = 80;
  const spacing = 210;
  const cy = yOffset + 40;

  return (
    <g>
      {steps.map((step, i) => {
        const cx = startX + i * spacing;
        if (i < steps.length - 1) {
          const nextCx = startX + (i + 1) * spacing;
          const lineId = `flow-${yOffset}-${i}`;
          return (
            <g key={`edge-${i}`}>
              <line
                x1={cx + nodeR + 4}
                y1={cy}
                x2={nextCx - nodeR - 4}
                y2={cy}
                stroke="var(--ck-edge-color)"
                strokeWidth="1.5"
              />
              <path
                id={lineId}
                d={`M${cx + nodeR + 4},${cy} L${nextCx - nodeR - 4},${cy}`}
                fill="none"
                stroke="none"
              />
              <circle r="3" fill={color} opacity="0.6">
                <animateMotion
                  dur="2s"
                  begin={`${i * 0.5}s`}
                  repeatCount="indefinite"
                >
                  <mpath href={`#${lineId}`} />
                </animateMotion>
                <animate
                  attributeName="opacity"
                  values="0;0.7;0.7;0"
                  keyTimes="0;0.15;0.85;1"
                  dur="2s"
                  begin={`${i * 0.5}s`}
                  repeatCount="indefinite"
                />
              </circle>
            </g>
          );
        }
        return null;
      })}

      {steps.map((step, i) => {
        const cx = startX + i * spacing;
        const isHovered = hoveredIdx === i;
        return (
          <g
            key={`node-${i}`}
            className="pipeline-viz-group"
            onMouseEnter={() => onHover(i)}
            onMouseLeave={() => onHover(null)}
            style={{ cursor: "default" }}
          >
            <circle
              cx={cx} cy={cy} r={nodeR}
              fill="var(--ck-surface)"
              className="pipeline-viz-node-bg"
            />
            <circle
              cx={cx} cy={cy} r={nodeR}
              className="pipeline-viz-node-ring"
              stroke={color}
              style={isHovered ? { opacity: 1, strokeWidth: 2.5 } : {}}
            />
            <text
              x={cx} y={cy + 1}
              textAnchor="middle"
              dominantBaseline="middle"
              className="pipeline-viz-num"
              fill={color}
            >
              {step.num}
            </text>
            <text
              x={cx} y={cy + nodeR + 18}
              textAnchor="middle"
              className="pipeline-viz-title"
            >
              {step.title}
            </text>
            {isHovered && (
              <foreignObject
                x={cx - 90} y={cy + nodeR + 28}
                width="180" height="60"
              >
                <div
                  xmlns="http://www.w3.org/1999/xhtml"
                  style={{
                    fontSize: "11px",
                    color: "var(--ck-text-muted)",
                    textAlign: "center",
                    lineHeight: 1.35,
                    fontFamily: "var(--ifm-font-family-base)",
                  }}
                >
                  {step.desc}
                </div>
              </foreignObject>
            )}
          </g>
        );
      })}
    </g>
  );
}

export default function PipelineViz() {
  const [hoveredIngest, setHoveredIngest] = useState<number | null>(null);
  const [hoveredRetrieve, setHoveredRetrieve] = useState<number | null>(null);

  const svgW = 780;
  const svgH = 280;

  return (
    <div className="pipeline-viz">
      <div className="pipeline-phase-label">Ingestion pipeline</div>
      <svg
        viewBox={`0 0 ${svgW} 140`}
        className="pipeline-viz-svg"
        preserveAspectRatio="xMidYMid meet"
      >
        <PipelineRow
          steps={ingestSteps}
          yOffset={0}
          color="var(--ck-accent)"
          hoveredIdx={hoveredIngest}
          onHover={setHoveredIngest}
        />
      </svg>

      <div className="pipeline-phase-label" style={{ marginTop: "1.5rem" }}>
        Retrieval pipeline
      </div>
      <svg
        viewBox={`0 0 ${svgW} 140`}
        className="pipeline-viz-svg"
        preserveAspectRatio="xMidYMid meet"
      >
        <PipelineRow
          steps={retrieveSteps}
          yOffset={0}
          color="var(--ck-entity-org)"
          hoveredIdx={hoveredRetrieve}
          onHover={setHoveredRetrieve}
        />
      </svg>
    </div>
  );
}
