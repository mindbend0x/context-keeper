import React from "react";
import Layout from "@theme/Layout";
import Link from "@docusaurus/Link";
import Tabs from "@theme/Tabs";
import TabItem from "@theme/TabItem";
import HeroGraph from "../components/HeroGraph";
import DemoTabs from "../components/DemoTabs";
import PipelineViz from "../components/PipelineViz";
import FeatureConstellation from "../components/FeatureConstellation";
import ToolTerminal from "../components/ToolTerminal";
import { useNodeReveal } from "../hooks/useNodeReveal";

function Hero() {
  return (
    <section className="hero-landing">
      <HeroGraph />
      <div style={{ maxWidth: 860, margin: "0 auto" }}>
        <div className="hero-badge">
          <span className="dot" /> Open source &middot; MIT License
        </div>
        <h1 className="hero-title">
          Memory that{" "}
          <span className="hero-accent">compounds</span>
        </h1>
        <p className="hero-sub">
          Every conversation starts from zero. CTX.K is a temporal knowledge
          graph that gives MCP-compatible assistants the ability to remember
          &mdash; entities, relationships, and how they change over time.
        </p>
        <div className="hero-actions">
          <Link className="btn btn-primary" to="/docs/getting-started">
            Get Started
          </Link>
          <a
            className="btn btn-secondary"
            href="https://github.com/mindbend0x/context-keeper"
            target="_blank"
            rel="noopener noreferrer"
          >
            View on GitHub
          </a>
        </div>
        <div className="hero-install">
          <span className="prompt">$</span>
          <code>brew install mindbend0x/context-keeper/context-keeper</code>
        </div>
      </div>
    </section>
  );
}

function DemoShowcase() {
  return (
    <section className="demo-showcase node-reveal" id="demo">
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

function HowItWorks() {
  return (
    <section className="landing-section node-reveal" id="how-it-works">
      <div className="section-label">How It Works</div>
      <h2 className="section-title">From text to temporal knowledge graph</h2>
      <p className="section-desc">
        Every piece of text your agent ingests becomes structured, searchable
        knowledge with a timeline.
      </p>
      <PipelineViz />
    </section>
  );
}

function Features() {
  return (
    <section className="landing-section node-reveal" id="features">
      <div className="section-label">Features</div>
      <h2 className="section-title">What makes it different</h2>
      <FeatureConstellation />
    </section>
  );
}

function McpTools() {
  return (
    <section className="landing-section node-reveal" id="mcp-tools">
      <div className="section-label">MCP Interface</div>
      <h2 className="section-title">10 tools your agent can call</h2>
      <p className="section-desc">
        Click any tool to see its description and sample response.
      </p>
      <ToolTerminal />
      <div style={{ textAlign: "center", marginTop: "1.5rem" }}>
        <Link className="btn btn-secondary" to="/docs/mcp-tools">
          Full MCP Reference &rarr;
        </Link>
      </div>
    </section>
  );
}

function QuickStart() {
  return (
    <section className="landing-section node-reveal" id="quickstart">
      <div className="section-label">Quick Start</div>
      <h2 className="section-title">Three commands to memory</h2>
      <p className="section-desc">
        Run Context Keeper locally via stdio, or connect to a remote instance
        over HTTP.
      </p>
      <div className="quickstart-code">
        <Tabs defaultValue="stdio" className="quickstart-tabs">
          <TabItem value="stdio" label="stdio (local)">
            <pre>
              <code>{`# Install the CLI
brew install mindbend0x/context-keeper/context-keeper

# Or build the MCP server from source
git clone https://github.com/mindbend0x/context-keeper.git && cd context-keeper
cargo build --release -p context-keeper-mcp

# Add to your MCP client config (Claude Desktop, Cursor, etc.)
{
  "mcpServers": {
    "context-keeper": {
      "command": "npx",
      "args": ["context-keeper-mcp"]
    }
  }
}

# Test with the CLI
context-keeper add --text "Alice is a senior engineer at Acme Corp"
context-keeper search --query "Who works at Acme?"`}</code>
            </pre>
          </TabItem>
          <TabItem value="http" label="HTTP (remote)">
            <pre>
              <code>{`# Start the HTTP server
MCP_TRANSPORT=http MCP_HTTP_PORT=3000 context-keeper-mcp

# Or run with Docker
docker compose up -d

# Point your MCP client to the HTTP endpoint
{
  "mcpServers": {
    "context-keeper": {
      "url": "http://localhost:3000/mcp"
    }
  }
}

# Test the connection
curl http://localhost:3000/mcp`}</code>
            </pre>
          </TabItem>
        </Tabs>
      </div>
      <div style={{ textAlign: "center", marginTop: "1.5rem" }}>
        <Link className="btn btn-primary" to="/docs/getting-started">
          Full Getting Started Guide &rarr;
        </Link>
      </div>
    </section>
  );
}

function Cta() {
  return (
    <section className="cta-band node-reveal">
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
          href="https://github.com/mindbend0x/context-keeper"
          target="_blank"
          rel="noopener noreferrer"
        >
          Star on GitHub
        </a>
      </div>
    </section>
  );
}

export default function Home(): React.JSX.Element {
  useNodeReveal();

  return (
    <Layout
      title="Persistent Memory for AI Agents"
      description="A temporal knowledge graph that gives MCP-compatible assistants long-term memory. Track entities, relationships, and changes over time. Built in Rust."
    >
      <Hero />
      <DemoShowcase />
      <HowItWorks />
      <Features />
      <McpTools />
      <QuickStart />
      <Cta />
    </Layout>
  );
}
