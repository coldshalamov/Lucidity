import {
  IPC_PROTOCOL_MAX_VERSION,
  IPC_PROTOCOL_MIN_VERSION,
  assertNegotiatedHello,
  unwrapResponse
} from "./protocol";
import type {
  DisposableLike,
  HostEvent,
  HostPort,
  HostPresentationSnapshot,
  HostRequest,
  HostResult,
  IpcRequest
} from "./protocol";
import { emptyPresentation } from "./model";
import type { ConnectionViewState } from "./model";

export interface Scheduler {
  schedule(callback: () => void, delayMilliseconds: number): DisposableLike;
}

export const systemScheduler: Scheduler = {
  schedule(callback, delayMilliseconds) {
    const handle = setTimeout(callback, delayMilliseconds);
    return { dispose: () => clearTimeout(handle) };
  }
};

export class HostClient implements DisposableLike {
  private nextRequestId = 1;
  private generation = 0;
  private reconnectAttempt = 0;
  private reconnectTimer: DisposableLike | undefined;
  private readonly disposables: DisposableLike[];
  private readonly snapshotListeners = new Set<(snapshot: HostPresentationSnapshot) => void>();
  private readonly connectionListeners = new Set<(state: ConnectionViewState) => void>();
  private currentSnapshot = emptyPresentation();
  private currentConnection: ConnectionViewState = {
    phase: "offline",
    message: "Lucidity host is offline.",
    attempt: 0
  };
  private disposed = false;

  public constructor(
    private readonly port: HostPort,
    private readonly scheduler: Scheduler = systemScheduler,
    private readonly reconnectDelayMilliseconds = 250
  ) {
    this.disposables = [
      port.onEvent((event) => {
        void this.handleEvent(event);
      }),
      port.onDisconnect((error) => {
        this.handleDisconnect(error);
      })
    ];
  }

  public get snapshot(): HostPresentationSnapshot {
    return this.currentSnapshot;
  }

  public get connection(): ConnectionViewState {
    return this.currentConnection;
  }

  public onSnapshot(listener: (snapshot: HostPresentationSnapshot) => void): DisposableLike {
    this.snapshotListeners.add(listener);
    listener(this.currentSnapshot);
    return { dispose: () => this.snapshotListeners.delete(listener) };
  }

  public onConnection(listener: (state: ConnectionViewState) => void): DisposableLike {
    this.connectionListeners.add(listener);
    listener(this.currentConnection);
    return { dispose: () => this.connectionListeners.delete(listener) };
  }

  public async start(): Promise<void> {
    await this.connect("connecting");
  }

  public async reconnectNow(): Promise<void> {
    this.reconnectTimer?.dispose();
    this.reconnectTimer = undefined;
    this.port.close();
    await this.connect("connecting");
  }

  public async request(request: HostRequest): Promise<HostResult> {
    if (this.currentConnection.phase !== "online") {
      throw new Error(
        "Lucidity host is offline. Start Lucidity Agent Terminal and run Lucidity: Reconnect to Host."
      );
    }
    const result = await this.send(request);
    this.refreshPresentation();
    return result;
  }

  public dispose(): void {
    if (this.disposed) {
      return;
    }
    this.disposed = true;
    this.generation += 1;
    this.reconnectTimer?.dispose();
    this.port.close();
    for (const disposable of this.disposables) {
      disposable.dispose();
    }
    this.snapshotListeners.clear();
    this.connectionListeners.clear();
  }

  private async connect(phase: "connecting" | "reconnecting"): Promise<void> {
    if (this.disposed) {
      return;
    }
    const generation = ++this.generation;
    this.setConnection({
      phase,
      message: phase === "connecting" ? "Connecting to Lucidity host…" : "Reconnecting to Lucidity host…",
      attempt: this.reconnectAttempt
    });
    try {
      await this.port.open();
      const hello = await this.send({
        method: "host.hello",
        params: {
          clientName: "lucidity-vscode",
          minVersion: IPC_PROTOCOL_MIN_VERSION,
          maxVersion: IPC_PROTOCOL_MAX_VERSION
        }
      });
      assertNegotiatedHello(hello);
      const page = await this.send({
        method: "conversation.list",
        params: { cursor: null, limit: 256 }
      });
      if (page.kind !== "conversations") {
        throw new Error(`conversation.list returned ${page.kind}, expected conversations`);
      }
      await this.send({ method: "event.subscribe" });
      if (generation !== this.generation || this.disposed) {
        return;
      }
      this.reconnectAttempt = 0;
      this.refreshPresentation();
      this.setConnection({ phase: "online", message: "Connected to Lucidity host.", attempt: 0 });
    } catch (error) {
      if (generation !== this.generation || this.disposed) {
        return;
      }
      this.setConnection({
        phase: "error",
        message: errorMessage(error),
        attempt: this.reconnectAttempt
      });
      this.scheduleReconnect();
      throw error;
    }
  }

  private async send(request: HostRequest): Promise<HostResult> {
    const id = this.nextRequestId++;
    const envelope = { id, ...request } as IpcRequest;
    return unwrapResponse(await this.port.request(envelope), id);
  }

  private async handleEvent(event: HostEvent): Promise<void> {
    if (this.currentConnection.phase !== "online") {
      return;
    }
    try {
      switch (event.event) {
        case "catalog.changed":
          await this.send({ method: "conversation.list", params: { cursor: null, limit: 256 } });
          break;
        case "conversation.changed":
          await this.send({
            method: "conversation.get",
            params: { conversationId: event.data.conversationId }
          });
          break;
        case "usage.changed":
          await this.send({
            method: "conversation.get",
            params: { conversationId: event.data.conversationId }
          });
          break;
        case "adapter.changed":
          await this.send({ method: "adapter.list" });
          break;
        case "runtime.attached":
        case "runtime.detached":
          break;
      }
      this.refreshPresentation();
    } catch (error) {
      this.handleDisconnect(error instanceof Error ? error : new Error(String(error)));
    }
  }

  private handleDisconnect(error: Error): void {
    if (this.disposed || this.currentConnection.phase === "offline") {
      return;
    }
    this.generation += 1;
    this.setConnection({
      phase: "offline",
      message: `${error.message} Start Lucidity Agent Terminal, then reconnect.`,
      attempt: this.reconnectAttempt
    });
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.disposed || this.reconnectTimer !== undefined) {
      return;
    }
    this.reconnectAttempt += 1;
    this.reconnectTimer = this.scheduler.schedule(() => {
      this.reconnectTimer = undefined;
      void this.connect("reconnecting").catch(() => {
        // connect records the precise error and schedules the next bounded retry.
      });
    }, this.reconnectDelayMilliseconds);
  }

  private refreshPresentation(): void {
    this.currentSnapshot = this.port.presentation();
    for (const listener of this.snapshotListeners) {
      listener(this.currentSnapshot);
    }
  }

  private setConnection(state: ConnectionViewState): void {
    this.currentConnection = state;
    for (const listener of this.connectionListeners) {
      listener(state);
    }
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
