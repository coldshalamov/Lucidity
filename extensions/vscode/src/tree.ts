import * as vscode from "vscode";
import { conversationsForSection, toTreeRow, usageStatusText, usageTooltip } from "./model";
import type { ConnectionPhase, ConversationSection } from "./model";
import type { HostPresentationSnapshot, UsageSnapshot } from "./protocol";

export class ConversationItem extends vscode.TreeItem {
  public readonly conversationId: string;
  public readonly conversationLabel: string;

  public constructor(row: ReturnType<typeof toTreeRow>) {
    super(row.label, vscode.TreeItemCollapsibleState.None);
    this.conversationId = row.id;
    this.conversationLabel = row.label;
    this.id = row.id;
    this.description = row.description;
    this.tooltip = row.tooltip;
    this.contextValue = row.contextValue;
    this.iconPath = new vscode.ThemeIcon(row.icon, new vscode.ThemeColor(row.themeColor));
    this.command = {
      command: "lucidity.openFocus",
      title: "Open or Focus Conversation",
      arguments: [this]
    };
    this.accessibilityInformation = {
      label: row.accessibilityLabel,
      role: "treeitem"
    };
  }
}

export class ConversationTreeProvider implements vscode.TreeDataProvider<ConversationItem> {
  private readonly changed = new vscode.EventEmitter<ConversationItem | undefined | void>();
  private snapshot: HostPresentationSnapshot;
  private currentWorkspacePath: string | undefined;
  public readonly onDidChangeTreeData = this.changed.event;

  public constructor(
    private readonly section: ConversationSection,
    initialSnapshot: HostPresentationSnapshot,
    currentWorkspacePath: string | undefined
  ) {
    this.snapshot = initialSnapshot;
    this.currentWorkspacePath = currentWorkspacePath;
  }

  public getTreeItem(element: ConversationItem): vscode.TreeItem {
    return element;
  }

  public getChildren(element?: ConversationItem): ConversationItem[] {
    if (element !== undefined) {
      return [];
    }
    return conversationsForSection(this.snapshot, this.section, this.currentWorkspacePath).map(
      (conversation) => new ConversationItem(toTreeRow(conversation))
    );
  }

  public update(snapshot: HostPresentationSnapshot, currentWorkspacePath: string | undefined): void {
    this.snapshot = snapshot;
    this.currentWorkspacePath = currentWorkspacePath;
    this.changed.fire();
  }

  public dispose(): void {
    this.changed.dispose();
  }
}

export class UsageStatusController implements vscode.Disposable {
  private readonly item = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 90);

  public constructor() {
    this.item.name = "Lucidity usage";
    this.item.command = "lucidity.refreshUsage";
    this.item.show();
  }

  public update(usage: UsageSnapshot, phase: ConnectionPhase): void {
    this.item.command = phase === "online" ? "lucidity.refreshUsage" : "lucidity.reconnect";
    this.item.text = usageStatusText(usage, phase);
    this.item.tooltip = usageTooltip(usage, phase);
    this.item.accessibilityInformation = {
      label:
        phase === "online"
          ? `Lucidity usage. ${usageStatusText(usage, phase).replace("$(pulse) ", "")}`
          : "Lucidity host offline. Activate to reconnect.",
      role: "button"
    };
  }

  public dispose(): void {
    this.item.dispose();
  }
}
