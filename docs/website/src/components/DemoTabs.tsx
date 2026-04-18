import React from "react";
import Tabs from "@theme/Tabs";
import TabItem from "@theme/TabItem";
import DemoVideo from "./DemoVideo";

interface DemoApp {
  name: string;
}

interface DemoEntry {
  value: string;
  label: string;
  apps: DemoApp[];
  src?: string;
  poster?: string;
  description: string;
  caption: string;
  alt: string;
  ctaLabel: string;
  ctaHref: string;
}

const demos: DemoEntry[] = [
  {
    value: "coding-agents",
    label: "Coding Agents",
    apps: [{ name: "Claude Code" }, { name: "Cursor" }],
    description:
      "Give your coding agent persistent memory across sessions. Store project context, architectural decisions, and conventions that survive between chats.",
    caption:
      "Agent stores context during a coding session, then recalls it in a later run.",
    alt: "Demo of Context Keeper inside a coding agent",
    ctaLabel: "Set up in your IDE",
    ctaHref: "/docs/tutorials/mcp-server-setup#claude-code",
  },
  {
    value: "mcp-apps",
    label: "MCP-Enabled Apps",
    apps: [
      { name: "Claude Desktop" },
      { name: "ChatGPT" },
      { name: "Perplexity" },
      { name: "Any MCP client" },
    ],
    description:
      "Works in any MCP-enabled chat app over stdio or HTTP. Persistent memory, shared across conversations.",
    caption: "One knowledge graph, every MCP client.",
    alt: "Demo of Context Keeper inside an MCP-enabled chat app",
    ctaLabel: "Connect an MCP client",
    ctaHref: "/docs/tutorials/mcp-server-setup",
  },
  {
    value: "cli",
    label: "CLI",
    apps: [{ name: "npx" }, { name: "Homebrew" }],
    description:
      "Install via npx or Homebrew. Three commands — add, search, inspect — to build and query your knowledge graph from the terminal.",
    caption: "Three commands: add, search, inspect. Zero boilerplate.",
    alt: "Demo of Context Keeper CLI in a terminal",
    ctaLabel: "CLI reference",
    ctaHref: "/docs/tutorials/cli-installation",
  },
];

export default function DemoTabs() {
  return (
    <div className="demo-tabs-wrapper">
      <Tabs defaultValue="coding-agents" className="demo-tabs" lazy={false}>
        {demos.map((d) => (
          <TabItem key={d.value} value={d.value} label={d.label}>
            <DemoVideo
              src={d.src}
              poster={d.poster}
              description={d.description}
              caption={d.caption}
              alt={d.alt}
              ctaLabel={d.ctaLabel}
              ctaHref={d.ctaHref}
              apps={d.apps}
            />
          </TabItem>
        ))}
      </Tabs>
    </div>
  );
}
