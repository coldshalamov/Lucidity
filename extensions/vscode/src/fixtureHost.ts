import fixture from "../fixtures/host.json";
import type {
  ConversationId,
  ConversationPresentation,
  DisposableLike,
  HostEvent,
  HostPort,
  HostPresentationSnapshot,
  HostResult,
  IpcRequest,
  IpcResponse,
  UsageSnapshot
} from "./protocol";

interface FixtureDocument {
  protocolVersion: number;
  hostInstanceId: string;
  conversations: ConversationPresentation[];
  usage: UsageSnapshot;
}

export class FixtureHost implements HostPort {
  private readonly eventListeners = new Set<(event: HostEvent) => void>();
  private readonly disconnectListeners = new Set<(error: Error) => void>();
  private readonly conversations: ConversationPresentation[];
  private usage: UsageSnapshot;
  private available = true;
  private connected = false;
  private subscribed = false;
  private newConversationSequence = 0;
  public readonly requests: IpcRequest[] = [];
  private readonly document: FixtureDocument;

  public constructor(currentWorkspacePath: string | undefined) {
    this.document = structuredClone(fixture) as FixtureDocument;
    const workspacePath = currentWorkspacePath ?? "C:/fixture/Lucidity";
    this.conversations = this.document.conversations.map((conversation) => ({
      ...conversation,
      projectPath:
        conversation.projectPath === "${workspace}" ? workspacePath : conversation.projectPath
    }));
    this.usage = structuredClone(this.document.usage);
  }

  public async open(): Promise<void> {
    if (!this.available) {
      throw new Error("Lucidity fixture host is unavailable.");
    }
    this.connected = true;
  }

  public close(): void {
    this.connected = false;
    this.subscribed = false;
  }

  public async request(request: IpcRequest): Promise<IpcResponse> {
    this.requests.push(structuredClone(request));
    if (!this.available || !this.connected) {
      throw new Error("Lucidity fixture host is offline.");
    }

    switch (request.method) {
      case "host.hello":
        if (
          request.params.minVersion > this.document.protocolVersion ||
          request.params.maxVersion < this.document.protocolVersion
        ) {
          return this.failure(request.id, "protocol_mismatch", "No shared protocol version.");
        }
        return this.success(request.id, {
          kind: "hello",
          data: {
            negotiatedVersion: this.document.protocolVersion,
            hostInstanceId: this.document.hostInstanceId,
            capabilities: ["events", "fixture-presentation-v1"]
          }
        });
      case "host.status":
        return this.success(request.id, {
          kind: "status",
          data: { hostInstanceId: this.document.hostInstanceId, shuttingDown: false }
        });
      case "host.openUi":
      case "adapter.scan":
        return this.success(request.id, { kind: "accepted" });
      case "host.quit":
        return this.success(request.id, { kind: "accepted" });
      case "conversation.list":
        return this.success(request.id, {
          kind: "conversations",
          data: {
            catalogSnapshotVersion: this.requests.length,
            conversations: this.conversations.map(stripPresentation),
            nextCursor: null,
            resyncRequired: false
          }
        });
      case "conversation.get": {
        const conversation = this.findConversation(request.params.conversationId);
        return this.success(request.id, {
          kind: "conversation",
          data: conversation === undefined ? null : stripPresentation(conversation)
        });
      }
      case "conversation.new": {
        const conversation = this.newConversation(request.params);
        this.conversations.unshift(conversation);
        this.emit({
          event: "catalog.changed",
          data: { catalogSnapshotVersion: this.requests.length }
        });
        return this.success(request.id, {
          kind: "conversation",
          data: stripPresentation(conversation)
        });
      }
      case "conversation.open": {
        const conversation = this.requireConversation(request.params.conversationId);
        for (const candidate of this.conversations) {
          candidate.attached = candidate.id === conversation.id;
        }
        this.emitConversationChanged(conversation.id);
        return this.success(request.id, { kind: "accepted" });
      }
      case "conversation.setOrganizationState": {
        const conversation = this.requireConversation(request.params.conversationId);
        conversation.organizationState = request.params.organizationState;
        this.emitConversationChanged(conversation.id);
        return this.success(request.id, { kind: "accepted" });
      }
      case "conversation.stop": {
        const conversation = this.requireConversation(request.params.conversationId);
        conversation.runtimeState = "not_running";
        conversation.attached = false;
        this.emitConversationChanged(conversation.id);
        return this.success(request.id, { kind: "accepted" });
      }
      case "conversation.refreshUsage":
        this.usage = { ...this.usage, stale: false, error: null };
        this.emit({ event: "usage.changed", data: { conversationId: request.params.conversationId } });
        return this.success(request.id, { kind: "accepted" });
      case "adapter.list":
        return this.success(request.id, { kind: "adapters", data: [] });
      case "event.subscribe":
        this.subscribed = true;
        return this.success(request.id, { kind: "accepted" });
    }
  }

  public presentation(): HostPresentationSnapshot {
    return {
      conversations: structuredClone(this.conversations),
      usage: structuredClone(this.usage)
    };
  }

  public onEvent(listener: (event: HostEvent) => void): DisposableLike {
    this.eventListeners.add(listener);
    return { dispose: () => this.eventListeners.delete(listener) };
  }

  public onDisconnect(listener: (error: Error) => void): DisposableLike {
    this.disconnectListeners.add(listener);
    return { dispose: () => this.disconnectListeners.delete(listener) };
  }

  public setAvailable(available: boolean): void {
    this.available = available;
    if (!available && this.connected) {
      this.connected = false;
      this.subscribed = false;
      for (const listener of this.disconnectListeners) {
        listener(new Error("Lucidity fixture host disconnected."));
      }
    }
  }

  public applyIncrementalPatch(
    conversationId: ConversationId,
    patch: Partial<Pick<ConversationPresentation, "runtimeState" | "eventTally" | "rowError">>
  ): void {
    Object.assign(this.requireConversation(conversationId), patch);
    this.emitConversationChanged(conversationId);
  }

  private success(id: number, payload: HostResult): IpcResponse {
    return { id, status: "ok", payload };
  }

  private failure(id: number, code: string, message: string): IpcResponse {
    return { id, status: "error", payload: { code, message, resyncRequired: true } };
  }

  private findConversation(id: ConversationId): ConversationPresentation | undefined {
    return this.conversations.find((conversation) => conversation.id === id);
  }

  private requireConversation(id: ConversationId): ConversationPresentation {
    const conversation = this.findConversation(id);
    if (conversation === undefined) {
      throw new Error(`Unknown fixture conversation ${id}`);
    }
    return conversation;
  }

  private newConversation(params: {
    adapterId: string;
    profileId: string;
    projectPath: string | null;
  }): ConversationPresentation {
    this.newConversationSequence += 1;
    const suffix = this.newConversationSequence.toString().padStart(12, "0");
    return {
      id: `99999999-9999-4999-8999-${suffix}`,
      native: {
        adapterId: params.adapterId,
        profileId: params.profileId,
        nativeSessionId: `fixture-new-${this.newConversationSequence}`
      },
      nativeSessionPath: null,
      title: "New Lucidity conversation",
      userAlias: null,
      projectPath: params.projectPath,
      createdAt: "2026-08-06T18:00:00Z",
      lastActivityAt: "2026-08-06T18:00:00Z",
      organizationState: "active",
      runtimeState: "starting",
      adapterDisplayName: "Fixture Agent",
      eventTally: 0,
      attached: false,
      pendingIdentity: true,
      evidenceStale: false,
      rowError: null
    };
  }

  private emitConversationChanged(conversationId: ConversationId): void {
    this.emit({ event: "conversation.changed", data: { conversationId } });
  }

  private emit(event: HostEvent): void {
    if (!this.subscribed) {
      return;
    }
    queueMicrotask(() => {
      for (const listener of this.eventListeners) {
        listener(event);
      }
    });
  }
}

function stripPresentation(conversation: ConversationPresentation) {
  return {
    id: conversation.id,
    native: conversation.native,
    nativeSessionPath: conversation.nativeSessionPath,
    title: conversation.title,
    userAlias: conversation.userAlias,
    projectPath: conversation.projectPath,
    createdAt: conversation.createdAt,
    lastActivityAt: conversation.lastActivityAt,
    organizationState: conversation.organizationState
  };
}
