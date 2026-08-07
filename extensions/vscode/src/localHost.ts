import { createConnection } from "node:net";
import type { Socket } from "node:net";
import { TextDecoder } from "node:util";
import { emptyPresentation } from "./model";
import type {
  ConversationPresentation,
  ConversationRecord,
  DisposableLike,
  HostEvent,
  HostPort,
  HostPresentationSnapshot,
  HostResult,
  IpcRequest,
  IpcResponse
} from "./protocol";

export const MAX_FRAME_BYTES = 1_048_576;
export const DEFAULT_PIPE_NAME = "lucidity-control-v1";

const LOCAL_PIPE_PREFIX = "\\\\.\\pipe\\";
const HOST_EVENTS = new Set<HostEvent["event"]>([
  "catalog.changed",
  "conversation.changed",
  "runtime.attached",
  "runtime.detached",
  "usage.changed",
  "adapter.changed"
]);

interface PendingRequest {
  resolve(response: IpcResponse): void;
  reject(error: Error): void;
}

interface PipeConnection {
  readonly socket: Socket;
  readonly openPromise: Promise<void>;
  readonly resolveOpen: () => void;
  readonly rejectOpen: (error: Error) => void;
  input: Buffer;
  connected: boolean;
  openSettled: boolean;
  intentionalClose: boolean;
  terminalError: Error | undefined;
}

interface AttachmentVersion {
  hostInstanceId: string;
  attachmentGeneration: number;
}

export function resolvePipePath(pipeName: string = DEFAULT_PIPE_NAME): string {
  const trimmed = pipeName.trim();
  if (trimmed.length === 0) {
    throw new Error("lucidity.pipeName must not be empty.");
  }
  if (trimmed.toLocaleLowerCase("en-US").startsWith(LOCAL_PIPE_PREFIX)) {
    if (trimmed.length === LOCAL_PIPE_PREFIX.length) {
      throw new Error("lucidity.pipeName must identify a pipe after the local pipe prefix.");
    }
    return trimmed;
  }
  if (trimmed.includes("\\") || trimmed.includes("/")) {
    throw new Error(
      "lucidity.pipeName must be a simple pipe name or a local \\\\.\\pipe\\ path; remote pipe paths are not allowed."
    );
  }
  return `${LOCAL_PIPE_PREFIX}${trimmed}`;
}

export function encodeJsonFrame(value: unknown): Buffer {
  const serialized = JSON.stringify(value);
  if (serialized === undefined) {
    throw new Error("Lucidity IPC messages must be JSON values.");
  }
  const body = Buffer.from(serialized, "utf8");
  if (body.length === 0 || body.length > MAX_FRAME_BYTES) {
    throw new Error(
      `Lucidity IPC frame length ${body.length} is outside the allowed 1..${MAX_FRAME_BYTES} bytes.`
    );
  }
  const frame = Buffer.allocUnsafe(4 + body.length);
  frame.writeUInt32LE(body.length, 0);
  body.copy(frame, 4);
  return frame;
}

export class NamedPipeHost implements HostPort {
  private readonly eventListeners = new Set<(event: HostEvent) => void>();
  private readonly disconnectListeners = new Set<(error: Error) => void>();
  private readonly pending = new Map<number, PendingRequest>();
  private readonly adapterNames = new Map<string, string>();
  private readonly attachmentVersions = new Map<string, AttachmentVersion>();
  private readonly decoder = new TextDecoder("utf-8", { fatal: true });
  private connection: PipeConnection | undefined;
  private snapshot: HostPresentationSnapshot = localEmptyPresentation();

  public readonly pipePath: string;

  public constructor(pipeName: string = DEFAULT_PIPE_NAME) {
    this.pipePath = resolvePipePath(pipeName);
  }

  public async open(): Promise<void> {
    if (process.platform !== "win32") {
      throw new Error("Lucidity local-host IPC requires Windows named pipes.");
    }
    const existing = this.connection;
    if (existing !== undefined) {
      return existing.openPromise;
    }

    let resolveOpen!: () => void;
    let rejectOpen!: (error: Error) => void;
    const openPromise = new Promise<void>((resolve, reject) => {
      resolveOpen = resolve;
      rejectOpen = reject;
    });
    const socket = createConnection(this.pipePath);
    const connection: PipeConnection = {
      socket,
      openPromise,
      resolveOpen,
      rejectOpen,
      input: Buffer.alloc(0),
      connected: false,
      openSettled: false,
      intentionalClose: false,
      terminalError: undefined
    };
    this.connection = connection;
    this.attachSocket(connection);
    return openPromise;
  }

  public close(): void {
    const connection = this.connection;
    if (connection === undefined) {
      return;
    }
    connection.intentionalClose = true;
    this.connection = undefined;
    const error = new Error("Lucidity named-pipe connection was closed by the client.");
    if (!connection.openSettled) {
      connection.openSettled = true;
      connection.rejectOpen(error);
    }
    this.rejectPending(error);
    connection.socket.destroy();
  }

  public request(request: IpcRequest): Promise<IpcResponse> {
    const connection = this.connection;
    if (connection === undefined || !connection.connected || connection.socket.destroyed) {
      return Promise.reject(new Error("Lucidity named-pipe host is not connected."));
    }
    if (!Number.isSafeInteger(request.id) || request.id < 0) {
      return Promise.reject(new Error(`Lucidity request id ${request.id} is not a safe integer.`));
    }
    if (this.pending.has(request.id)) {
      return Promise.reject(new Error(`Lucidity request id ${request.id} is already pending.`));
    }

    let frame: Buffer;
    try {
      frame = encodeJsonFrame(request);
    } catch (error) {
      return Promise.reject(asError(error));
    }

    return new Promise<IpcResponse>((resolve, reject) => {
      this.pending.set(request.id, {
        resolve: (response) => {
          try {
            this.applyResponse(request, response);
            resolve(response);
          } catch (error) {
            reject(asError(error));
            throw error;
          }
        },
        reject
      });
      try {
        connection.socket.write(frame, (error?: Error | null) => {
          if (error === undefined || error === null) {
            return;
          }
          const pending = this.pending.get(request.id);
          if (pending !== undefined) {
            this.pending.delete(request.id);
            pending.reject(withPipeContext(error, this.pipePath));
          }
        });
      } catch (error) {
        this.pending.delete(request.id);
        reject(withPipeContext(asError(error), this.pipePath));
      }
    });
  }

  public presentation(): HostPresentationSnapshot {
    return structuredClone(this.snapshot);
  }

  public onEvent(listener: (event: HostEvent) => void): DisposableLike {
    this.eventListeners.add(listener);
    return { dispose: () => this.eventListeners.delete(listener) };
  }

  public onDisconnect(listener: (error: Error) => void): DisposableLike {
    this.disconnectListeners.add(listener);
    return { dispose: () => this.disconnectListeners.delete(listener) };
  }

  private attachSocket(connection: PipeConnection): void {
    connection.socket.once("connect", () => {
      connection.connected = true;
      if (!connection.openSettled) {
        connection.openSettled = true;
        connection.resolveOpen();
      }
    });
    connection.socket.on("data", (chunk: Buffer) => {
      try {
        this.consume(connection, chunk);
      } catch (error) {
        this.failConnection(connection, asError(error));
      }
    });
    connection.socket.once("error", (error: Error) => {
      connection.terminalError ??= withPipeContext(error, this.pipePath);
      if (!connection.openSettled) {
        connection.openSettled = true;
        connection.rejectOpen(connection.terminalError);
      }
    });
    connection.socket.once("close", () => {
      const wasConnected = connection.connected;
      connection.connected = false;
      if (this.connection === connection) {
        this.connection = undefined;
      }
      const error =
        connection.terminalError ??
        new Error(`Lucidity named-pipe host disconnected (${this.pipePath}).`);
      if (!connection.openSettled) {
        connection.openSettled = true;
        connection.rejectOpen(error);
      }
      this.rejectPending(error);
      if (!connection.intentionalClose && wasConnected) {
        for (const listener of this.disconnectListeners) {
          listener(error);
        }
      }
    });
  }

  private consume(connection: PipeConnection, chunk: Buffer): void {
    connection.input =
      connection.input.length === 0 ? chunk : Buffer.concat([connection.input, chunk]);
    while (connection.input.length >= 4) {
      const length = connection.input.readUInt32LE(0);
      if (length === 0 || length > MAX_FRAME_BYTES) {
        throw new Error(
          `Lucidity IPC frame length ${length} is outside the allowed 1..${MAX_FRAME_BYTES} bytes.`
        );
      }
      if (connection.input.length < 4 + length) {
        return;
      }
      const body = connection.input.subarray(4, 4 + length);
      connection.input = connection.input.subarray(4 + length);
      let value: unknown;
      try {
        value = JSON.parse(this.decoder.decode(body)) as unknown;
      } catch (error) {
        const cause = asError(error);
        throw new Error(`Lucidity IPC received invalid UTF-8 JSON: ${cause.message}`, {
          cause: error
        });
      }
      this.routeMessage(value);
    }
  }

  private routeMessage(value: unknown): void {
    if (isIpcResponse(value)) {
      const pending = this.pending.get(value.id);
      if (pending === undefined) {
        throw new Error(`Lucidity IPC received an unexpected response id ${value.id}.`);
      }
      this.pending.delete(value.id);
      pending.resolve(value);
      return;
    }
    if (isHostEvent(value)) {
      this.applyEvent(value);
      for (const listener of this.eventListeners) {
        listener(value);
      }
      return;
    }
    throw new Error("Lucidity IPC received a message that is neither a response nor a v1 event.");
  }

  private failConnection(connection: PipeConnection, error: Error): void {
    connection.terminalError ??= withPipeContext(error, this.pipePath);
    connection.socket.destroy();
  }

  private rejectPending(error: Error): void {
    for (const pending of this.pending.values()) {
      pending.reject(error);
    }
    this.pending.clear();
  }

  private applyResponse(request: IpcRequest, response: IpcResponse): void {
    if (response.status !== "ok") {
      return;
    }
    const result = response.payload;
    switch (result.kind) {
      case "conversations":
        this.applyConversationPage(request, result);
        break;
      case "conversation":
        this.applyConversation(
          request,
          result,
          request.method === "conversation.new" ? "starting" : undefined
        );
        break;
      case "adapters":
        this.applyAdapters(result.data);
        break;
      case "hello":
      case "accepted":
      case "status":
        break;
    }
  }

  private applyConversationPage(
    request: IpcRequest,
    result: Extract<HostResult, { kind: "conversations" }>
  ): void {
    if (request.method !== "conversation.list") {
      return;
    }
    const existing = new Map(
      this.snapshot.conversations.map((conversation) => [conversation.id, conversation])
    );
    const page = result.data.conversations.map((record) =>
      this.toPresentation(record, existing.get(record.id))
    );
    if (request.params.cursor === null) {
      this.snapshot = { ...this.snapshot, conversations: page };
      return;
    }
    const merged = [...this.snapshot.conversations];
    for (const conversation of page) {
      const index = merged.findIndex((candidate) => candidate.id === conversation.id);
      if (index === -1) {
        merged.push(conversation);
      } else {
        merged[index] = conversation;
      }
    }
    this.snapshot = { ...this.snapshot, conversations: merged };
  }

  private applyConversation(
    request: IpcRequest,
    result: Extract<HostResult, { kind: "conversation" }>,
    fallbackRuntimeState?: ConversationPresentation["runtimeState"]
  ): void {
    const record = result.data;
    if (record === null) {
      if (request.method === "conversation.get") {
        this.snapshot = {
          ...this.snapshot,
          conversations: this.snapshot.conversations.filter(
            (conversation) => conversation.id !== request.params.conversationId
          )
        };
      }
      return;
    }
    const conversations = [...this.snapshot.conversations];
    const index = conversations.findIndex((candidate) => candidate.id === record.id);
    const previous = index === -1 ? undefined : conversations[index];
    const presentation = this.toPresentation(record, previous, fallbackRuntimeState);
    if (index === -1) {
      conversations.unshift(presentation);
    } else {
      conversations[index] = presentation;
    }
    this.snapshot = { ...this.snapshot, conversations };
  }

  private applyAdapters(adapters: unknown[]): void {
    for (const adapter of adapters) {
      if (!isRecord(adapter) || typeof adapter.id !== "string" || typeof adapter.displayName !== "string") {
        continue;
      }
      this.adapterNames.set(adapter.id, adapter.displayName);
    }
    this.snapshot = {
      ...this.snapshot,
      conversations: this.snapshot.conversations.map((conversation) => ({
        ...conversation,
        adapterDisplayName:
          this.adapterNames.get(conversation.native.adapterId) ?? conversation.adapterDisplayName
      }))
    };
  }

  private applyEvent(event: HostEvent): void {
    if (event.event === "runtime.attached") {
      const attachment = event.data.attachment;
      this.attachmentVersions.set(attachment.conversationId, {
        hostInstanceId: attachment.hostInstanceId,
        attachmentGeneration: attachment.attachmentGeneration
      });
      this.patchConversation(attachment.conversationId, {
        attached: true,
        runtimeState: "unknown_external"
      });
      return;
    }
    if (event.event === "runtime.detached") {
      const current = this.attachmentVersions.get(event.data.conversationId);
      if (
        current !== undefined &&
        current.hostInstanceId === event.data.version.hostInstanceId &&
        current.attachmentGeneration === event.data.version.attachmentGeneration
      ) {
        this.attachmentVersions.delete(event.data.conversationId);
        this.patchConversation(event.data.conversationId, {
          attached: false,
          runtimeState: "not_running"
        });
      }
    }
  }

  private patchConversation(
    conversationId: string,
    patch: Partial<Pick<ConversationPresentation, "attached" | "runtimeState">>
  ): void {
    this.snapshot = {
      ...this.snapshot,
      conversations: this.snapshot.conversations.map((conversation) =>
        conversation.id === conversationId ? { ...conversation, ...patch } : conversation
      )
    };
  }

  private toPresentation(
    record: ConversationRecord,
    previous?: ConversationPresentation,
    fallbackRuntimeState: ConversationPresentation["runtimeState"] = "not_running"
  ): ConversationPresentation {
    return {
      ...record,
      runtimeState: previous?.runtimeState ?? fallbackRuntimeState,
      adapterDisplayName:
        this.adapterNames.get(record.native.adapterId) ??
        previous?.adapterDisplayName ??
        record.native.adapterId,
      eventTally: previous?.eventTally ?? 0,
      attached: previous?.attached ?? false,
      pendingIdentity: previous?.pendingIdentity ?? false,
      evidenceStale: previous?.evidenceStale ?? false,
      rowError: previous?.rowError ?? null
    };
  }
}

function localEmptyPresentation(): HostPresentationSnapshot {
  const snapshot = emptyPresentation();
  return {
    ...snapshot,
    usage: {
      ...snapshot.usage,
      source: "local-protocol-v1",
      stale: true,
      error: "Protocol v1 does not expose a complete usage snapshot."
    }
  };
}

function isIpcResponse(value: unknown): value is IpcResponse {
  if (!isRecord(value) || !Number.isSafeInteger(value.id) || typeof value.status !== "string") {
    return false;
  }
  if (value.status === "ok") {
    return isRecord(value.payload) && typeof value.payload.kind === "string";
  }
  return (
    value.status === "error" &&
    isRecord(value.payload) &&
    typeof value.payload.code === "string" &&
    typeof value.payload.message === "string" &&
    typeof value.payload.resyncRequired === "boolean"
  );
}

function isHostEvent(value: unknown): value is HostEvent {
  return (
    isRecord(value) &&
    typeof value.event === "string" &&
    HOST_EVENTS.has(value.event as HostEvent["event"]) &&
    isRecord(value.data)
  );
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function asError(error: unknown): Error {
  return error instanceof Error ? error : new Error(String(error));
}

function withPipeContext(error: Error, pipePath: string): Error {
  return new Error(`Lucidity named pipe ${pipePath}: ${error.message}`, { cause: error });
}
