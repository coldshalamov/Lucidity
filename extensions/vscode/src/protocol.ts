export const IPC_PROTOCOL_MIN_VERSION = 1;
export const IPC_PROTOCOL_MAX_VERSION = 1;

export type ConversationId = string;
export type ProfileId = string;
export type HostInstanceId = string;
export type AdapterId = string;

export type OrganizationState = "active" | "settled";
export type RuntimeState =
  | "not_running"
  | "starting"
  | "working"
  | "waiting_for_input"
  | "awaiting_approval"
  | "completed_idle"
  | "failed"
  | "unknown_external";

export interface NativeConversationKey {
  adapterId: AdapterId;
  profileId: ProfileId;
  nativeSessionId: string;
}

export interface ConversationRecord {
  id: ConversationId;
  native: NativeConversationKey;
  nativeSessionPath: string | null;
  title: string;
  userAlias: string | null;
  projectPath: string | null;
  createdAt: string;
  lastActivityAt: string;
  organizationState: OrganizationState;
}

export interface CatalogCursor {
  catalogSnapshotVersion: number;
  lastActivityAt: string;
  conversationId: ConversationId;
}

export interface ConversationPage {
  catalogSnapshotVersion: number;
  conversations: ConversationRecord[];
  nextCursor: CatalogCursor | null;
  resyncRequired: boolean;
}

export interface HostHelloResponse {
  negotiatedVersion: number;
  hostInstanceId: HostInstanceId;
  capabilities: string[];
}

export type HostRequest =
  | {
      method: "host.hello";
      params: { clientName: string; minVersion: number; maxVersion: number };
    }
  | { method: "host.status" }
  | { method: "host.openUi" }
  | { method: "host.quit" }
  | { method: "conversation.list"; params: { cursor: CatalogCursor | null; limit: number } }
  | { method: "conversation.get"; params: { conversationId: ConversationId } }
  | {
      method: "conversation.new";
      params: { adapterId: AdapterId; profileId: ProfileId; projectPath: string | null };
    }
  | { method: "conversation.open"; params: { conversationId: ConversationId } }
  | {
      method: "conversation.setOrganizationState";
      params: { conversationId: ConversationId; organizationState: OrganizationState };
    }
  | { method: "conversation.stop"; params: { conversationId: ConversationId } }
  | { method: "conversation.refreshUsage"; params: { conversationId: ConversationId } }
  | { method: "adapter.list" }
  | { method: "adapter.scan" }
  | { method: "event.subscribe" };

export type HostResult =
  | { kind: "hello"; data: HostHelloResponse }
  | { kind: "accepted" }
  | { kind: "conversation"; data: ConversationRecord | null }
  | { kind: "conversations"; data: ConversationPage }
  | { kind: "adapters"; data: unknown[] }
  | {
      kind: "status";
      data: { hostInstanceId: HostInstanceId; shuttingDown: boolean };
    };

export type IpcRequest = HostRequest & { id: number };

export type IpcResponse =
  | { id: number; status: "ok"; payload: HostResult }
  | {
      id: number;
      status: "error";
      payload: { code: string; message: string; resyncRequired: boolean };
    };

export type HostEvent =
  | { event: "catalog.changed"; data: { catalogSnapshotVersion: number } }
  | { event: "conversation.changed"; data: { conversationId: ConversationId } }
  | {
      event: "runtime.attached";
      data: {
        attachment: {
          conversationId: ConversationId;
          paneId: number;
          processId: number | null;
          processOwner: unknown | null;
          hostInstanceId: HostInstanceId;
          attachmentGeneration: number;
          attachedAt: string;
        };
      };
    }
  | {
      event: "runtime.detached";
      data: {
        conversationId: ConversationId;
        version: { hostInstanceId: HostInstanceId; attachmentGeneration: number };
      };
    }
  | { event: "usage.changed"; data: { conversationId: ConversationId } }
  | { event: "adapter.changed"; data: { adapterId: AdapterId } };

export interface UsageMeasure {
  value: number;
  limit: number;
  unit: string;
}

export interface UsageSnapshot {
  conversationId: ConversationId | null;
  quota: UsageMeasure | null;
  context: UsageMeasure | null;
  observedAt: string;
  source: string;
  stale: boolean;
  error: string | null;
}

export interface ConversationPresentation extends ConversationRecord {
  runtimeState: RuntimeState;
  adapterDisplayName: string;
  eventTally: number;
  attached: boolean;
  pendingIdentity: boolean;
  evidenceStale: boolean;
  rowError: string | null;
}

export interface HostPresentationSnapshot {
  conversations: ConversationPresentation[];
  usage: UsageSnapshot;
}

export interface DisposableLike {
  dispose(): void;
}

export interface HostPort {
  open(): Promise<void>;
  close(): void;
  request(request: IpcRequest): Promise<IpcResponse>;
  presentation(): HostPresentationSnapshot;
  onEvent(listener: (event: HostEvent) => void): DisposableLike;
  onDisconnect(listener: (error: Error) => void): DisposableLike;
}

export function assertNegotiatedHello(result: HostResult): HostHelloResponse {
  if (result.kind !== "hello") {
    throw new Error(`host.hello returned ${result.kind}, expected hello`);
  }
  if (
    result.data.negotiatedVersion < IPC_PROTOCOL_MIN_VERSION ||
    result.data.negotiatedVersion > IPC_PROTOCOL_MAX_VERSION
  ) {
    throw new Error(`unsupported Lucidity protocol version ${result.data.negotiatedVersion}`);
  }
  return result.data;
}

export function unwrapResponse(response: IpcResponse, requestId: number): HostResult {
  if (response.id !== requestId) {
    throw new Error(`response id ${response.id} did not match request ${requestId}`);
  }
  if (response.status === "error") {
    throw new Error(`${response.payload.code}: ${response.payload.message}`);
  }
  return response.payload;
}
