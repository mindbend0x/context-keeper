import * as vscode from "vscode";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";

// ---------------------------------------------------------------------------
// MCP client lifecycle
// ---------------------------------------------------------------------------

let mcpClient: Client | undefined;

async function ensureClient(): Promise<Client> {
  if (mcpClient) {
    return mcpClient;
  }

  const config = vscode.workspace.getConfiguration("contextKeeper");
  const binaryPath = config.get<string>("binaryPath", "context-keeper-mcp");
  const storagePath = config.get<string>("storagePath", "");

  const args: string[] = [];
  const env: Record<string, string> = { ...process.env } as Record<
    string,
    string
  >;
  if (storagePath) {
    env["STORAGE_BACKEND"] = `rocksdb:${storagePath}`;
  }

  const transport = new StdioClientTransport({
    command: binaryPath,
    args,
    env,
  });

  const client = new Client(
    { name: "context-keeper-cursor", version: "0.1.0" },
    { capabilities: {} },
  );

  await client.connect(transport);
  mcpClient = client;
  return client;
}

async function callTool(
  name: string,
  args: Record<string, unknown>,
): Promise<string> {
  const client = await ensureClient();
  const result = await client.callTool({ name, arguments: args });

  // Extract text from content array
  const texts = (result.content as Array<{ type: string; text?: string }>)
    .filter((c) => c.type === "text" && c.text)
    .map((c) => c.text!);
  return texts.join("\n");
}

// ---------------------------------------------------------------------------
// Sidebar tree view — recent memories
// ---------------------------------------------------------------------------

interface MemoryItem {
  content: string;
  created_at: string;
}

class MemoryTreeItem extends vscode.TreeItem {
  constructor(public readonly memory: MemoryItem) {
    // Show first 80 chars as the label
    const label =
      memory.content.length > 80
        ? memory.content.slice(0, 80) + "…"
        : memory.content;
    super(label, vscode.TreeItemCollapsibleState.None);
    this.tooltip = memory.content;
    this.description = new Date(memory.created_at).toLocaleString();
  }
}

class RecentMemoriesProvider
  implements vscode.TreeDataProvider<MemoryTreeItem>
{
  private _onDidChangeTreeData = new vscode.EventEmitter<
    MemoryTreeItem | undefined
  >();
  readonly onDidChangeTreeData = this._onDidChangeTreeData.event;

  refresh(): void {
    this._onDidChangeTreeData.fire(undefined);
  }

  async getChildren(): Promise<MemoryTreeItem[]> {
    try {
      const raw = await callTool("list_recent", { limit: 20 });
      const items: MemoryItem[] = JSON.parse(raw);
      return items.map((m) => new MemoryTreeItem(m));
    } catch (err) {
      vscode.window.showWarningMessage(
        `Context Keeper: failed to fetch recent memories — ${err}`,
      );
      return [];
    }
  }

  getTreeItem(element: MemoryTreeItem): vscode.TreeItem {
    return element;
  }
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

async function searchMemoryCommand() {
  const query = await vscode.window.showInputBox({
    prompt: "Search the knowledge graph",
    placeHolder: "e.g. authentication flow",
  });
  if (!query) {
    return;
  }

  try {
    const raw = await callTool("search_memory", { query, limit: 10 });
    const results = JSON.parse(raw) as Array<{
      name: string;
      entity_type: string;
      summary: string;
      score: number;
    }>;

    if (results.length === 0) {
      vscode.window.showInformationMessage("No results found.");
      return;
    }

    const picks = results.map((r) => ({
      label: `$(symbol-class) ${r.name}`,
      description: r.entity_type,
      detail: r.summary,
    }));

    await vscode.window.showQuickPick(picks, {
      title: `Memory search: "${query}"`,
      matchOnDetail: true,
    });
  } catch (err) {
    vscode.window.showErrorMessage(
      `Context Keeper: search failed — ${err}`,
    );
  }
}

async function addMemoryCommand() {
  const editor = vscode.window.activeTextEditor;
  if (!editor) {
    vscode.window.showWarningMessage("No active editor.");
    return;
  }

  const selection = editor.document.getText(editor.selection);
  if (!selection) {
    vscode.window.showWarningMessage("No text selected.");
    return;
  }

  const source = `cursor:${vscode.workspace.asRelativePath(editor.document.uri)}`;

  try {
    const raw = await callTool("add_memory", { text: selection, source });
    const result = JSON.parse(raw);
    vscode.window.showInformationMessage(
      `Remembered: ${result.entities_created} entities, ${result.relations_created} relations`,
    );
  } catch (err) {
    vscode.window.showErrorMessage(
      `Context Keeper: add memory failed — ${err}`,
    );
  }
}

// ---------------------------------------------------------------------------
// Extension lifecycle
// ---------------------------------------------------------------------------

export function activate(context: vscode.ExtensionContext) {
  const recentProvider = new RecentMemoriesProvider();

  vscode.window.registerTreeDataProvider(
    "contextKeeper.recentMemories",
    recentProvider,
  );

  context.subscriptions.push(
    vscode.commands.registerCommand(
      "contextKeeper.searchMemory",
      searchMemoryCommand,
    ),
    vscode.commands.registerCommand(
      "contextKeeper.addMemory",
      addMemoryCommand,
    ),
    vscode.commands.registerCommand("contextKeeper.refreshRecent", () =>
      recentProvider.refresh(),
    ),
  );

  // Auto-refresh sidebar on activation
  recentProvider.refresh();
}

export function deactivate() {
  if (mcpClient) {
    mcpClient.close().catch(() => {});
    mcpClient = undefined;
  }
}
