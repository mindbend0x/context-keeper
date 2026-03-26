import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const sidebars: SidebarsConfig = {
  docs: [
    {
      type: "category",
      label: "Getting Started",
      collapsed: false,
      items: ["getting-started"],
    },
    {
      type: "category",
      label: "Concepts",
      collapsed: false,
      items: ["how-it-works", "use-cases"],
    },
    {
      type: "category",
      label: "Architecture",
      collapsed: false,
      items: ["architecture", "adr-001"],
    },
    {
      type: "category",
      label: "Reference",
      collapsed: false,
      items: ["mcp-tools", "cli-reference", "configuration"],
    },
    {
      type: "category",
      label: "Community",
      collapsed: false,
      items: ["contributing"],
    },
  ],
};

export default sidebars;
