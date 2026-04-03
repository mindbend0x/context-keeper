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
  caption: string;
  alt: string;
  ctaLabel: string;
  ctaHref: string;
}

const demos: DemoEntry[] = [
  {
    value: "cursor",
    label: "Cursor",
    icon: "⌨️",
    caption:
      "Watch an agent store a memory during a coding session, then recall it three conversations later.",
    alt: "Demo of Context Keeper running inside Cursor IDE",
    ctaLabel: "Set up Cursor",
    ctaHref: "/docs/getting-started#mcp-client-configuration",
  },
  {
    value: "claude",
    label: "Claude Desktop",
    icon: "💬",
    caption:
      "Claude remembers your preferences across conversations — no prompt engineering required.",
    alt: "Demo of Context Keeper running inside Claude Desktop",
    ctaLabel: "Set up Claude Desktop",
    ctaHref: "/docs/getting-started#mcp-client-configuration",
  },
  {
    value: "cli",
    label: "CLI",
    icon: "▸",
    caption: "Three commands: add, search, inspect. Zero boilerplate.",
    alt: "Demo of Context Keeper CLI in a terminal",
    ctaLabel: "CLI reference",
    ctaHref: "/docs/cli-reference",
  },
  {
    value: "http",
    label: "HTTP API",
    icon: "⇄",
    caption:
      "Multiple agents sharing one knowledge graph over HTTP.",
    alt: "Demo of Context Keeper HTTP API with multiple agents",
    ctaLabel: "HTTP transport docs",
    ctaHref: "/docs/configuration#transport",
  },
];

export default function DemoTabs() {
  return (
    <div className="demo-tabs-wrapper">
      <Tabs defaultValue="cursor" className="demo-tabs" lazy={false}>
        {demos.map((d) => (
          <TabItem
            key={d.value}
            value={d.value}
            label={`${d.icon}  ${d.label}`}
          >
            <DemoVideo
              src={d.src}
              poster={d.poster}
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
