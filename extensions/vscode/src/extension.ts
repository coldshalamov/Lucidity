import * as vscode from "vscode";
import { HostClient } from "./client";
import { createCommandHandlers } from "./commands";
import type { ConversationCommandTarget } from "./commands";
import { FixtureHost } from "./fixtureHost";
import { DEFAULT_PIPE_NAME, NamedPipeHost } from "./localHost";
import { emptyPresentation } from "./model";
import { ConversationTreeProvider, UsageStatusController } from "./tree";
import type { ConversationItem } from "./tree";
import type { HostPort } from "./protocol";

export function activate(context: vscode.ExtensionContext): void {
  const initialWorkspace = currentWorkspacePath();
  const port = createHostPort(initialWorkspace);
  const client = new HostClient(port);
  const providers = {
    currentWorkspace: new ConversationTreeProvider(
      "currentWorkspace",
      emptyPresentation(),
      initialWorkspace
    ),
    otherProjects: new ConversationTreeProvider(
      "otherProjects",
      emptyPresentation(),
      initialWorkspace
    ),
    settled: new ConversationTreeProvider("settled", emptyPresentation(), initialWorkspace)
  };
  const views = [
    vscode.window.createTreeView("lucidity.currentWorkspace", {
      treeDataProvider: providers.currentWorkspace,
      canSelectMany: false
    }),
    vscode.window.createTreeView("lucidity.otherProjects", {
      treeDataProvider: providers.otherProjects,
      canSelectMany: false
    }),
    vscode.window.createTreeView("lucidity.settled", {
      treeDataProvider: providers.settled,
      canSelectMany: false
    })
  ];
  const usageStatus = new UsageStatusController();
  const handlers = createCommandHandlers(client, {
    workspacePath: currentWorkspacePath,
    adapterId: () => configuration().get("defaultAdapterId", "fixture-adapter"),
    profileId: () =>
      configuration().get("defaultProfileId", "22222222-2222-4222-8222-222222222222"),
    async confirmStop(label) {
      const selection = await vscode.window.showWarningMessage(
        `Stop ${label}?`,
        { modal: true, detail: "Settling is non-destructive; stopping terminates the live agent." },
        "Stop"
      );
      return selection === "Stop";
    },
    inform: (message) => {
      void vscode.window.showInformationMessage(message);
    }
  });

  const updateViews = (): void => {
    const workspace = currentWorkspacePath();
    providers.currentWorkspace.update(client.snapshot, workspace);
    providers.otherProjects.update(client.snapshot, workspace);
    providers.settled.update(client.snapshot, workspace);
    usageStatus.update(client.snapshot.usage, client.connection.phase);
  };

  const command = <T extends unknown[]>(
    id: string,
    callback: (...arguments_: T) => Promise<void>
  ): vscode.Disposable =>
    vscode.commands.registerCommand(id, (...arguments_: T) =>
      callback(...arguments_).catch((error: unknown) => {
        void vscode.window.showErrorMessage(error instanceof Error ? error.message : String(error));
      })
    );

  context.subscriptions.push(
    ...views,
    ...Object.values(providers),
    usageStatus,
    client,
    client.onSnapshot(updateViews),
    client.onConnection((state) => {
      void vscode.commands.executeCommand("setContext", "lucidity.connected", state.phase === "online");
      const message =
        state.phase === "online"
          ? ""
          : `${state.message} Run “Lucidity: Reconnect to Host” to retry now.`;
      for (const view of views) {
        view.message = message;
      }
      usageStatus.update(client.snapshot.usage, state.phase);
    }),
    vscode.workspace.onDidChangeWorkspaceFolders(updateViews),
    vscode.window.onDidChangeActiveTextEditor(updateViews),
    command("lucidity.newInCurrentWorkspace", handlers.newInCurrentWorkspace),
    command("lucidity.openFocus", async (item?: ConversationItem) =>
      handlers.openFocus(requireTarget(item))
    ),
    command("lucidity.settle", async (item?: ConversationItem) =>
      handlers.settle(requireTarget(item))
    ),
    command("lucidity.unsettle", async (item?: ConversationItem) =>
      handlers.unsettle(requireTarget(item))
    ),
    command("lucidity.stop", async (item?: ConversationItem) => handlers.stop(requireTarget(item))),
    command("lucidity.refreshUsage", async (item?: ConversationItem) =>
      handlers.refreshUsage(item === undefined ? undefined : toTarget(item))
    ),
    command("lucidity.openLucidity", handlers.openLucidity),
    command("lucidity.reconnect", handlers.reconnect)
  );

  updateViews();
  void client.start().catch((error: unknown) => {
    void vscode.window.showWarningMessage(
      `${error instanceof Error ? error.message : String(error)} Use Lucidity: Reconnect to Host after the host is available.`
    );
  });
}

function createHostPort(workspacePath: string | undefined): HostPort {
  return configuration().get<"fixture" | "local">("hostMode", "fixture") === "fixture"
    ? new FixtureHost(workspacePath)
    : new NamedPipeHost(configuration().get("pipeName", DEFAULT_PIPE_NAME));
}

function configuration(): vscode.WorkspaceConfiguration {
  return vscode.workspace.getConfiguration("lucidity");
}

function currentWorkspacePath(): string | undefined {
  const activeUri = vscode.window.activeTextEditor?.document.uri;
  if (activeUri !== undefined) {
    const activeFolder = vscode.workspace.getWorkspaceFolder(activeUri);
    if (activeFolder !== undefined) {
      return activeFolder.uri.fsPath;
    }
  }
  return vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
}

function requireTarget(item: ConversationItem | undefined): ConversationCommandTarget {
  if (item === undefined) {
    throw new Error("Select a Lucidity conversation before running this command.");
  }
  return toTarget(item);
}

function toTarget(item: ConversationItem): ConversationCommandTarget {
  return { id: item.conversationId, label: item.conversationLabel };
}
