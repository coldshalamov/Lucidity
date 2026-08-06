import { emptyPresentation } from "./model";
import type {
  DisposableLike,
  HostEvent,
  HostPort,
  HostPresentationSnapshot,
  IpcRequest,
  IpcResponse
} from "./protocol";

export class UnavailableLocalHost implements HostPort {
  public async open(): Promise<void> {
    throw new Error(
      "The local Lucidity IPC transport is not available in this build. Start the fixture host or install the host-integration update."
    );
  }

  public close(): void {}

  public async request(_request: IpcRequest): Promise<IpcResponse> {
    void _request;
    throw new Error("The local Lucidity IPC transport is unavailable.");
  }

  public presentation(): HostPresentationSnapshot {
    return emptyPresentation();
  }

  public onEvent(_listener: (event: HostEvent) => void): DisposableLike {
    void _listener;
    return { dispose() {} };
  }

  public onDisconnect(_listener: (error: Error) => void): DisposableLike {
    void _listener;
    return { dispose() {} };
  }
}
