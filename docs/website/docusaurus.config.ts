import { themes as prismThemes } from "prism-react-renderer";
import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

const config: Config = {
  title: "Context Keeper",
  tagline: "Persistent memory for AI agents",
  favicon: "img/favicon.ico",

  url: process.env.SITE_URL || "https://mindbend0x.github.io",
  baseUrl: process.env.SITE_BASE_URL || "/context-keeper/",

  organizationName: "mindbend0x",
  projectName: "context-keeper",

  onBrokenLinks: "throw",
  onBrokenMarkdownLinks: "warn",

  markdown: {
    mermaid: true,
  },

  themes: ["@docusaurus/theme-mermaid"],

  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },

  presets: [
    [
      "classic",
      {
        docs: {
          sidebarPath: "./sidebars.ts",
          editUrl:
            "https://github.com/mindbend0x/context-keeper/tree/main/docs/website/",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      defaultMode: "dark",
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },

    navbar: {
      title: "CTX.K",
      style: "dark",
      items: [
        {
          type: "docSidebar",
          sidebarId: "docs",
          position: "left",
          label: "Docs",
        },
        {
          href: "https://github.com/mindbend0x/context-keeper",
          label: "GitHub",
          position: "right",
        },
      ],
    },

    footer: {
      style: "dark",
      links: [
        {
          title: "Docs",
          items: [
            { label: "Getting Started", to: "/docs/getting-started" },
            { label: "Architecture", to: "/docs/architecture" },
            { label: "MCP Reference", to: "/docs/mcp-tools" },
          ],
        },
        {
          title: "Reference",
          items: [
            { label: "CLI", to: "/docs/cli-reference" },
            { label: "Configuration", to: "/docs/configuration" },
            { label: "ADR-001", to: "/docs/adr-001" },
          ],
        },
        {
          title: "Community",
          items: [
            {
              label: "GitHub",
              href: "https://github.com/mindbend0x/context-keeper",
            },
            { label: "Contributing", to: "/docs/contributing" },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Context Keeper Contributors. MIT License.`,
    },

    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: ["bash", "json", "toml", "rust"],
    },

    mermaid: {
      theme: { light: "default", dark: "dark" },
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
