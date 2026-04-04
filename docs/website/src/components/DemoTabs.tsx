import React from "react";
import Tabs from "@theme/Tabs";
import TabItem from "@theme/TabItem";
import DemoVideo from "./DemoVideo";

interface DemoEntry {
  value: string;
  label: string;
  icon: string;
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
    value: "claude-code",
    label: "Claude Code",
    icon: ">_",
    description:
      "Context Keeper gives your cowork agent persistent memory across sessions. Store project context, architectural decisions, and team knowledge that persists between tasks.",
    caption:
      "Agent stores context during a coding session, then recalls it in a later cowork run.",
    alt: "Demo of Context Keeper running inside Claude Code",
    ctaLabel: "Set up Claude Code",
    ctaHref: "/docs/tutorials/mcp-server-setup#claude-code",
  },
  {
    value: "cursor",
    label: "Cursor",
    icon: "⌨️",
    description:
      "Give Cursor's agent memory of your codebase context, past decisions, and project conventions. Memories persist across chat sessions and composer runs.",
    caption:
      "Watch an agent store a memory during a coding session, then recall it three conversations later.",
    alt: "Demo of Context Keeper running inside Cursor IDE",
    ctaLabel: "Set up Cursor",
    ctaHref: "/docs/tutorials/mcp-server-setup#cursor",
  },
  {
    value: "claude-desktop",
    label: "Claude Desktop",
    icon: "💬",
    description:
      "Claude remembers your preferences, past conversations, and context across sessions — no prompt engineering required. Just talk naturally.",
    caption:
      "Claude remembers your preferences across conversations — no prompt engineering required.",
    alt: "Demo of Context Keeper running inside Claude Desktop",
    ctaLabel: "Set up Claude Desktop",
    ctaHref: "/docs/tutorials/mcp-server-setup#claude-desktop",
  },
  {
    value: "chatgpt",
    label: "ChatGPT",
    icon: "🤖",
    description:
      "Add persistent memory to ChatGPT via HTTP transport. Store and recall context across conversations, giving ChatGPT long-term memory capabilities.",
    caption:
      "ChatGPT gains persistent memory via Context Keeper's HTTP transport.",
    alt: "Demo of Context Keeper running with ChatGPT",
    ctaLabel: "Set up ChatGPT",
    ctaHref: "/docs/tutorials/mcp-server-setup#chatgpt",
  },
  {
    value: "perplexity",
    label: "Perplexity",
    icon: "🔍",
    description:
      "Enhance Perplexity with persistent research context. Build up knowledge over multiple research sessions, so each query builds on what you've already explored.",
    caption:
      "Perplexity retains research context across sessions for deeper investigations.",
    alt: "Demo of Context Keeper running with Perplexity",
    ctaLabel: "Set up Perplexity",
    ctaHref: "/docs/tutorials/mcp-server-setup#perplexity",
  },
  {
    value: "cli",
    label: "CLI",
    icon: "▸",
    description:
      "Three commands: add, search, inspect. Build and query your knowledge graph directly from the terminal. Perfect for scripting and automation.",
    caption: "Three commands: add, search, inspect. Zero boilerplate.",
    alt: "Demo of Context Keeper CLI in a terminal",
    ctaLabel: "CLI reference",
    ctaHref: "/docs/tutorials/cli-installation",
  },
  {
    value: "http",
    label: "MCP (HTTP)",
    icon: "⇄",
    description:
      "Run Context Keeper as an HTTP server and let multiple agents read and write to the same knowledge graph. Ideal for multi-agent architectures and Docker deployments.",
    caption:
      "Multiple agents sharing one knowledge graph over HTTP.",
    alt: "Demo of Context Keeper HTTP API with multiple agents",
    ctaLabel: "HTTP transport docs",
    ctaHref: "/docs/tutorials/http-transport",
  },
];

export default function DemoTabs() {
  return (
    <div className="demo-tabs-wrapper">
      <Tabs defaultValue="claude-code" className="demo-tabs" lazy={false}>
        {demos.map((d) => (
          <TabItem
            key={d.value}
            value={d.value}
            label={d.label}
          >
            <DemoVideo
              src={d.src}
              poster={d.poster}
              description={d.description}
              caption={d.caption}
              alt={d.alt}
              ctaLabel={d.ctaLabel}
              ctaHref={d.ctaHref}
            />
          </TabItem>
        ))}
      </Tabs>
    </div>
  );
}
