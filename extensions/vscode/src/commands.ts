import type { HostPresentationSnapshot, HostRequest, HostResult } from "./protocol";

export interface ConversationCommandTarget {
  id: string;
  label?: string;
}

export interface HostCommandClient {
  readonly snapshot: HostPresentationSnapshot;
  request(request: HostRequest): Promise<HostResult>;
  reconnectNow(): Promise<void>;
}

export interface CommandEnvironment {
  workspacePath(): string | undefined;
  adapterId(): string;
  profileId(): string;
  confirmStop(label: string): Promise<boolean>;
  inform(message: string): void;
}

export interface LucidityCommandHandlers {
  newInCurrentWorkspace(): Promise<void>;
  openFocus(target: ConversationCommandTarget): Promise<void>;
  settle(target: ConversationCommandTarget): Promise<void>;
  unsettle(target: ConversationCommandTarget): Promise<void>;
  stop(target: ConversationCommandTarget): Promise<void>;
  refreshUsage(target?: ConversationCommandTarget): Promise<void>;
  openLucidity(): Promise<void>;
  reconnect(): Promise<void>;
}

export function createCommandHandlers(
  client: HostCommandClient,
  environment: CommandEnvironment
): LucidityCommandHandlers {
  return {
    async newInCurrentWorkspace() {
      const projectPath = environment.workspacePath();
      if (projectPath === undefined) {
        throw new Error("Open a workspace folder before starting a Lucidity conversation.");
      }
      const adapterId = environment.adapterId().trim();
      const profileId = environment.profileId().trim();
      if (adapterId.length === 0 || profileId.length === 0) {
        throw new Error("Configure both lucidity.defaultAdapterId and lucidity.defaultProfileId.");
      }
      await client.request({
        method: "conversation.new",
        params: { adapterId, profileId, projectPath }
      });
    },

    async openFocus(target) {
      await client.request({
        method: "conversation.open",
        params: { conversationId: target.id }
      });
    },

    async settle(target) {
      await client.request({
        method: "conversation.setOrganizationState",
        params: { conversationId: target.id, organizationState: "settled" }
      });
    },

    async unsettle(target) {
      await client.request({
        method: "conversation.setOrganizationState",
        params: { conversationId: target.id, organizationState: "active" }
      });
    },

    async stop(target) {
      const confirmed = await environment.confirmStop(target.label ?? "this conversation");
      if (!confirmed) {
        return;
      }
      await client.request({
        method: "conversation.stop",
        params: { conversationId: target.id }
      });
    },

    async refreshUsage(target) {
      const conversationId = target?.id ?? client.snapshot.usage.conversationId;
      if (conversationId === null || conversationId === undefined) {
        environment.inform("No attached conversation has usage data to refresh.");
        return;
      }
      await client.request({
        method: "conversation.refreshUsage",
        params: { conversationId }
      });
    },

    async openLucidity() {
      await client.request({ method: "host.openUi" });
    },

    async reconnect() {
      await client.reconnectNow();
    }
  };
}
