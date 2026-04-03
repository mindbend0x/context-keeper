import React, { useEffect } from "react";
import Layout from "@theme/Layout";
import Link from "@docusaurus/Link";
import DemoTabs from "../components/DemoTabs";

// ── Scroll reveal hook ──────────────────────────────────────────────
function useScrollReveal() {
  useEffect(() => {
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((e) => {
          if (e.isIntersecting) e.target.classList.add("visible");
        });
      },
      { threshold: 0.08 }
    );
    document.querySelectorAll(".reveal").forEach((el) => observer.observe(el));
    return () => observer.disconnect();
  }, []);
}

// ── Hero ────────────────────────────────────────────────────────────
function Hero() {
  return (
    <section className="hero-landing">
      <div style={{ maxWidth: 800, margin: "0 auto" }}>
        <div className="hero-badge">
          <span className="dot" /> Open source &middot; MIT License
        </div>
        <h1 className="hero-title">
          Persistent memory for
          <br />
          <span className="gradient-animated">AI agents</span>
        </h1>
        <p className="hero-sub">
          A temporal knowledge graph that gives MCP-compatible assistants the
          ability to remember. Entities, relationships, and how they change over
          time &mdash; searchable, snapshotable, and built in Rust.
        </p>
        <div className="hero-actions">
          <Link className="btn btn-primary" to="/docs/getting-started">
            Get Started
          </Link>
          <a
            className="btn btn-secondary"
            href="https://github.com/0x313/context-keeper"
            target="_blank"
            rel="noopener noreferrer"
          >
            View on GitHub
          </a>
        </div>
        <div className="hero-install">
          <span className="prompt">$</span>
          <code>cargo install context-keeper-mcp</code>
          <span style={{ fontSize: "0.7rem", color: "var(--ck-text-muted)", marginLeft: "0.25rem" }}>(soon)</span>
        </div>
      </div>
    </section>
  );
}

// ── Problem ─────────────────────────────────────────────────────────
function Problem() {
  return (
    <section className="landing-section reveal" style={{ paddingBottom: "1rem" }}>
      <div style={{ maxWidth: 720, margin: "0 auto", textAlign: "center" }}>
        <div className="section-label">The Problem</div>
        <h2 className="section-title">AI assistants forget everything</h2>
        <p className="section-desc">
          Every conversation starts from zero. Your agent doesn't know what it
          learned yesterday, can't track how facts change over time, and has no
          way to connect related information across sessions. Context windows are
          a cache, not a memory.
        </p>
      </div>
    </section>
  );
}

// ── Demo Showcase ──────────────────────────────────────────────────
function DemoShowcase() {
  return (
    <section className="demo-showcase reveal" id="demo">
      <div className="section-label">See It In Action</div>
      <h2 className="section-title">One tool, every client</h2>
      <p className="section-desc">
        Context Keeper works wherever MCP does. Pick your client and see
        persistent memory in action.
      </p>
      <DemoTabs />
    </section>
  );
}

// ── Pipeline step component ────────────────────────────────────────
function PipelineStep({
  num,
  title,
  desc,
  showArrow = true,
}: {
  num: string;
  title: string;
  desc: string;
  showArrow?: boolean;
}) {
  return (
    <div className="pipeline-step">
      <div className="step-num">{num}</div>
      <h4>{title}</h4>
      <p>{desc}</p>
      {showArrow && (
        <div className="pipeline-arrow">&rarr;</div>
      )}
    </div>
  );
}

// ── How it works ────────────────────────────────────────────────────
function HowItWorks() {
  return (
    <section className="landing-section reveal" id="how-it-works">
      <div className="section-label">How It Works</div>
      <h2 className="section-title">From text to temporal knowledge graph</h2>
      <p className="section-desc">
        Every piece of text your agent ingests becomes structured, searchable
        knowledge with a timeline.
      </p>

      <div className="pipeline">
        <PipelineStep num="1" title="Ingest" desc="Agent sends text via add_memory. Source is tagged (chat, document, code)." />
        <PipelineStep num="2" title="Extract" desc="LLM extracts entities (people, orgs, concepts) and relationships between them." />
        <PipelineStep num="3" title="Embed" desc="Vector embeddings generated for semantic search. BM25 index updated for keywords." />
        <PipelineStep num="4" title="Store" desc="Entities upserted with temporal bounds. Relations merged. Graph evolves over time." showArrow={false} />
      </div>

      <div className="section-label" style={{ marginTop: "2.5rem" }}>
        Then, when the agent needs to remember
      </div>

      <div className="pipeline">
        <PipelineStep num="A" title="Search" desc="Hybrid vector + keyword search fused with Reciprocal Rank Fusion (RRF)." />
        <PipelineStep num="B" title="Expand" desc="LLM rewrites the query into semantic variants, each searched independently." />
        <PipelineStep num="C" title="Traverse" desc="Follow entity relationships through the graph. See who connects to what." />
        <PipelineStep num="D" title="Snapshot" desc="Query the graph at any point in time. See what was true last Tuesday." showArrow={false} />
      </div>
    </section>
  );
}

// ── Use cases ───────────────────────────────────────────────────────
const useCases = [
  { icon: "\u{1F4AC}", title: "Conversational Memory", desc: "Give your chat agent persistent memory across sessions. It remembers preferences, past decisions, and context without stuffing everything into the prompt.", tags: ["Claude Desktop", "Cursor"] },
  { icon: "\u{1F4DA}", title: "Knowledge Base for RAG", desc: "Ingest documents, meeting notes, and wikis into a structured graph. Hybrid search retrieves better context than vector-only RAG.", tags: ["retrieval-augmented generation"] },
  { icon: "\u{1F50D}", title: "Codebase Intelligence", desc: "Feed your agent context about your codebase: who owns what, architectural decisions, dependency relationships.", tags: ["developer tools"] },
  { icon: "\u{23F3}", title: "Temporal Audit Trail", desc: "Track how facts change over time. When did Alice move teams? Snapshot any point in time and diff it against the present.", tags: ["compliance", "change tracking"] },
  { icon: "\u{1F916}", title: "Multi-Agent Shared Memory", desc: "Run Context Keeper over HTTP and let multiple agents read and write to the same knowledge graph.", tags: ["HTTP transport", "Docker"] },
  { icon: "\u{1F9E9}", title: "Personal Knowledge Graph", desc: "Build a structured map of your notes, contacts, and projects. Search by concept, not just keywords.", tags: ["personal productivity"] },
];

function UseCases() {
  return (
    <section className="landing-section reveal" id="use-cases">
      <div className="section-label">Use Cases</div>
      <h2 className="section-title">Built for agents that need to remember</h2>
      <div className="usecase-grid">
        {useCases.map((uc, i) => (
          <div className="usecase-card" key={i}>
            <div className="icon">{uc.icon}</div>
            <h3>{uc.title}</h3>
            <p>{uc.desc}</p>
            {uc.tags.map((t) => (
              <span className="usecase-tag" key={t}>{t}</span>
            ))}
          </div>
        ))}
      </div>
    </section>
  );
}

// ── Features ────────────────────────────────────────────────────────
const features = [
  { icon: "\u{1F570}\u{FE0F}", title: "Temporal Knowledge Graph", desc: "Every entity and relation carries valid_from / valid_until timestamps. Point-in-time snapshots let you query the graph at any moment." },
  { icon: "\u{1F50E}", title: "Hybrid Search + RRF", desc: "HNSW vector similarity and BM25 keyword search, fused with Reciprocal Rank Fusion (K=60). LLM-powered query expansion." },
  { icon: "\u{1F517}", title: "MCP Native", desc: "10 tools, browsable entity resources, and 3 prompt templates. Works with Claude Desktop, Cursor, and any MCP-compatible client." },
  { icon: "\u{2699}\u{FE0F}", title: "Trait-Based Architecture", desc: "Core defines pure traits for embedders, extractors, and query rewriters. Swap providers without touching the pipeline." },
  { icon: "\u{1F4BE}", title: "SurrealDB All-in-One", desc: "One database for documents, graph edges, vector indexes, and full-text search. No middleware glue." },
  { icon: "\u{1F433}", title: "Ship Anywhere", desc: "Run as an MCP server (stdio or HTTP), a CLI tool, or a Docker container. RocksDB persistence by default." },
];

function Features() {
  return (
    <section className="landing-section reveal" id="features">
      <div className="section-label">Features</div>
      <h2 className="section-title">What makes it different</h2>
      <div className="feature-grid">
        {features.map((f, i) => (
          <div className="feature-card" key={i}>
            <div className="icon">{f.icon}</div>
            <h3>{f.title}</h3>
            <p>{f.desc}</p>
          </div>
        ))}
      </div>
      <div className="stats-row">
        {[
          ["5", "Rust crates"],
          ["10", "MCP tools"],
          ["35+", "DB operations"],
          ["0", "API keys to test"],
        ].map(([v, l]) => (
          <div className="stat" key={l}>
            <div className="stat-value">{v}</div>
            <div className="stat-label">{l}</div>
          </div>
        ))}
      </div>
    </section>
  );
}

// ── Architecture ────────────────────────────────────────────────────
function Architecture() {
  return (
    <section className="landing-section reveal" id="architecture">
      <div className="section-label">Architecture</div>
      <h2 className="section-title">Five-crate Rust workspace</h2>
      <p className="section-desc">
        Pure logic in core. LLM integrations in rig. Storage in surreal. CLI and
        MCP are thin binaries that wire it together.
      </p>
      <div className="arch-visual">
        {/* Row 1: Binaries */}
        <div className="arch-row arch-row-top">
          <div className="arch-node arch-bin">
            <span className="arch-node-icon">⌨️</span>
            <span className="arch-node-name">cli</span>
            <span className="arch-node-sub">Developer CLI</span>
          </div>
          <div className="arch-node arch-bin">
            <span className="arch-node-icon">🔌</span>
            <span className="arch-node-name">mcp</span>
            <span className="arch-node-sub">MCP server</span>
          </div>
        </div>

        {/* Connector lines */}
        <div className="arch-connectors">
          <svg width="100%" height="48" viewBox="0 0 600 48" preserveAspectRatio="xMidYMid meet">
            <line x1="150" y1="0" x2="300" y2="44" stroke="var(--ck-accent)" strokeWidth="2" strokeDasharray="6 4" opacity="0.5" />
            <line x1="450" y1="0" x2="300" y2="44" stroke="var(--ck-accent)" strokeWidth="2" strokeDasharray="6 4" opacity="0.5" />
          </svg>
        </div>

        {/* Row 2: Core */}
        <div className="arch-row arch-row-center">
          <div className="arch-node arch-core">
            <span className="arch-node-icon">⚙️</span>
            <span className="arch-node-name">core</span>
            <span className="arch-node-sub">Models · Pipeline · Search · Traits</span>
          </div>
        </div>

        {/* Connector lines */}
        <div className="arch-connectors">
          <svg width="100%" height="48" viewBox="0 0 600 48" preserveAspectRatio="xMidYMid meet">
            <line x1="200" y1="4" x2="200" y2="44" stroke="var(--ck-accent)" strokeWidth="2" strokeDasharray="6 4" opacity="0.5" />
            <line x1="400" y1="4" x2="400" y2="44" stroke="var(--ck-accent)" strokeWidth="2" strokeDasharray="6 4" opacity="0.5" />
          </svg>
        </div>

        {/* Row 3: Implementations */}
        <div className="arch-row arch-row-bottom">
          <div className="arch-node arch-impl">
            <span className="arch-node-icon">🧠</span>
            <span className="arch-node-name">rig</span>
            <span className="arch-node-sub">LLM integrations</span>
          </div>
          <div className="arch-node arch-impl">
            <span className="arch-node-icon">💾</span>
            <span className="arch-node-name">surreal</span>
            <span className="arch-node-sub">SurrealDB · 35+ methods</span>
          </div>
        </div>
      </div>
    </section>
  );
}

// ── MCP Tools ───────────────────────────────────────────────────────
const tools = [
  { name: "add_memory", desc: "Ingest text, extract entities and relations, store everything with embeddings. Returns a diff of what changed." },
  { name: "search_memory", desc: "Hybrid vector + keyword search with RRF fusion. Filter by entity type." },
  { name: "expand_search", desc: "LLM rewrites your query into semantic variants, searches each, and merges results." },
  { name: "get_entity", desc: "Look up any entity by name. Get its type, summary, temporal bounds, and relationships." },
  { name: "snapshot", desc: "Point-in-time graph state. Pass a timestamp, get back every entity and relation." },
  { name: "list_recent", desc: "The N most recent memories, ordered by creation time." },
  { name: "list_agents", desc: "See which AI agents have contributed to the graph, with namespaces and episode counts." },
  { name: "list_namespaces", desc: "List all namespaces in the graph with entity counts for multi-tenant visibility." },
  { name: "agent_activity", desc: "Audit a specific agent's recent contributions by agent_id." },
  { name: "cross_namespace_search", desc: "Search across all namespaces globally, ignoring namespace scoping." },
];

function McpTools() {
  return (
    <section className="landing-section reveal" id="mcp-tools">
      <div className="section-label">MCP Interface</div>
      <h2 className="section-title">10 tools your agent can call</h2>
      <div className="tools-grid">
        {tools.map((t, i) => (
          <div className="tool-card" key={i}>
            <h3>{t.name}</h3>
            <p>{t.desc}</p>
          </div>
        ))}
      </div>
      <div style={{ textAlign: "center", marginTop: "1.5rem" }}>
        <Link className="btn btn-secondary" to="/docs/mcp-tools">
          Full MCP Reference →
        </Link>
      </div>
    </section>
  );
}

// ── Quick Start ─────────────────────────────────────────────────────
function QuickStart() {
  return (
    <section className="landing-section reveal" id="quickstart">
      <div className="section-label">Quick Start</div>
      <h2 className="section-title">Three commands to memory</h2>
      <div className="quickstart-code">
        <pre>
          <code>{`# Install
cargo install context-keeper-mcp

# Add to your MCP client config (Claude Desktop, Cursor, etc.)
{
  "mcpServers": {
    "context-keeper": {
      "command": "context-keeper-mcp"
    }
  }
}

# Or test with the CLI
cargo install context-keeper-cli
context-keeper add --text "Alice is a senior engineer at Acme Corp"
context-keeper search --query "Who works at Acme?"`}</code>
        </pre>
      </div>
      <div style={{ textAlign: "center", marginTop: "1.5rem" }}>
        <Link className="btn btn-primary" to="/docs/getting-started">
          Full Getting Started Guide →
        </Link>
      </div>
    </section>
  );
}

// ── CTA ─────────────────────────────────────────────────────────────
function Cta() {
  return (
    <section className="cta-band reveal">
      <h2 className="section-title">Ready to give your agent a memory?</h2>
      <p className="section-desc" style={{ marginBottom: "1.5rem" }}>
        Open source. Rust-fast. Drop it into any MCP client.
      </p>
      <div className="hero-actions">
        <Link className="btn btn-primary" to="/docs/getting-started">
          Get Started
        </Link>
        <a
          className="btn btn-secondary"
          href="https://github.com/0x313/context-keeper"
          target="_blank"
          rel="noopener noreferrer"
        >
          Star on GitHub
        </a>
      </div>
    </section>
  );
}

// ── Page ────────────────────────────────────────────────────────────
export default function Home(): React.JSX.Element {
  useScrollReveal();

  return (
    <Layout
      title="Persistent Memory for AI Agents"
      description="A temporal knowledge graph that gives MCP-compatible assistants long-term memory. Track entities, relationships, and changes over time. Built in Rust."
    >
      <Hero />
      <Problem />
      <DemoShowcase />
      <HowItWorks />
      <UseCases />
      <Features />
      <Architecture />
      <McpTools />
      <QuickStart />
      <Cta />
    </Layout>
  );
}
