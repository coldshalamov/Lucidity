import assert from "node:assert/strict";
import { randomUUID } from "node:crypto";
import { createServer } from "node:net";
import type { Server, Socket } from "node:net";
import test from "node:test";
import { HostClient } from "../src/client";
import type { Scheduler } from "../src/client";
import {
  DEFAULT_PIPE_NAME,
  MAX_FRAME_BYTES,
  NamedPipeHost,
  resolvePipePath
} from "../src/localHost";
import type {
  CatalogCursor,
  ConversationRecord,
  DisposableLike,
  HostEvent,
  HostResult,
  IpcRequest,
  IpcResponse
} from "../src/protocol";

const windowsTest = process.platform === "win32" ? test : test.skip;

class ManualScheduler implements Scheduler {
  private readonly callbacks: Array<{ callback: () => void; disposed: boolean }> = [];

  public get pending(): number {
    return this.callbacks.filter((entry) => !entry.disposed).length;
  }

  public schedule(callback: () => void, _delayMilliseconds: number): DisposableLike {
    void _delayMilliseconds;
    const entry = { callback, disposed: false };
    this.callbacks.push(entry);
    return { dispose: () => (entry.disposed = true) };
  }

  public runNext(): void {
    const entry = this.callbacks.find((candidate) => !candidate.disposed);
    if (entry === undefined) {
      assert.fail("no scheduled callback was available");
    }
    entry.disposed = true;
    entry.callback();
  }
}

void test("pipe paths use one documented local default and reject remote paths", () => {
  assert.equal(resolvePipePath(), `\\\\.\\pipe\\${DEFAULT_PIPE_NAME}`);
  assert.equal(resolvePipePath("test-pipe"), "\\\\.\\pipe\\test-pipe");
  assert.equal(resolvePipePath("\\\\.\\pipe\\test-pipe"), "\\\\.\\pipe\\test-pipe");
  assert.throws(() => resolvePipePath("\\\\remote-host\\pipe\\lucidity"), /remote pipe paths/);
  assert.throws(() => resolvePipePath("  "), /must not be empty/);
});

void windowsTest("real named pipe fragments frames and correlates concurrent responses by id", async (t) => {
  const harness = await createPipeHarness();
  t.after(async () => harness.close());
  const host = new NamedPipeHost(harness.pipePath);
  let disconnectCount = 0;
  host.onDisconnect(() => {
    disconnectCount += 1;
  });
  await host.open();
  const socket = await harness.nextSocket();

  const first = host.request({ id: 41, method: "host.status" });
  const second = host.request({ id: 42, method: "host.status" });
  await waitFor(() => harness.requests.length === 2);
  assert.deepEqual(
    harness.requests.map((request) => request.id),
    [41, 42]
  );

  const combined = Buffer.concat([
    responseFrame(42, {
      kind: "status",
      data: { hostInstanceId: "host-42", shuttingDown: false }
    }),
    responseFrame(41, {
      kind: "status",
      data: { hostInstanceId: "host-41", shuttingDown: false }
    })
  ]);
  socket.write(combined.subarray(0, 2));
  socket.write(combined.subarray(2));

  assert.equal((await first).id, 41);
  assert.equal((await second).id, 42);

  const receivedEvent = new Promise<HostEvent>((resolve) => {
    const disposable = host.onEvent((event) => {
      disposable.dispose();
      resolve(event);
    });
  });
  socket.write(
    independentFrame({
      event: "catalog.changed",
      data: { catalogSnapshotVersion: 9 }
    } satisfies HostEvent)
  );
  assert.deepEqual(await receivedEvent, {
    event: "catalog.changed",
    data: { catalogSnapshotVersion: 9 }
  });

  await assert.rejects(
    host.request({
      id: 43,
      method: "conversation.new",
      params: {
        adapterId: "fixture",
        profileId: "profile",
        projectPath: "x".repeat(MAX_FRAME_BYTES)
      }
    }),
    /outside the allowed/
  );

  const pending = host.request({ id: 44, method: "host.status" });
  await waitFor(() => harness.requests.some((request) => request.id === 44));
  host.close();
  await assert.rejects(pending, /closed by the client/);
  assert.equal(disconnectCount, 0, "intentional close must not look like a host failure");
});

void windowsTest("real named pipe performs paginated snapshots and full reconnect replacement", async (t) => {
  const harness = await createPipeHarness((socket, request, connectionIndex) => {
    if (request.method === "host.hello") {
      socket.write(
        responseFrame(request.id, {
          kind: "hello",
          data: {
            negotiatedVersion: 1,
            hostInstanceId: `host-${connectionIndex}`,
            capabilities: ["events"]
          }
        })
      );
      return;
    }
    if (request.method === "conversation.list") {
      const firstPage = request.params.cursor === null;
      const page =
        connectionIndex === 1
          ? firstPage
            ? {
                conversations: [conversation("a", "First page")],
                nextCursor: cursor("a")
              }
            : { conversations: [conversation("b", "Second page")], nextCursor: null }
          : { conversations: [conversation("c", "Fresh reconnect")], nextCursor: null };
      socket.write(
        responseFrame(request.id, {
          kind: "conversations",
          data: {
            catalogSnapshotVersion: connectionIndex,
            conversations: page.conversations,
            nextCursor: page.nextCursor,
            resyncRequired: false
          }
        })
      );
      return;
    }
    if (request.method === "event.subscribe") {
      socket.write(responseFrame(request.id, { kind: "accepted" }));
      return;
    }
    assert.fail(`unexpected request ${request.method}`);
  });
  t.after(async () => harness.close());
  const scheduler = new ManualScheduler();
  const host = new NamedPipeHost(harness.pipePath);
  const client = new HostClient(host, scheduler, 1);
  t.after(() => client.dispose());

  await client.start();
  assert.equal(client.connection.phase, "online");
  assert.deepEqual(
    client.snapshot.conversations.map((item) => item.title),
    ["First page", "Second page"]
  );
  assert.deepEqual(harness.requests.slice(0, 4).map((request) => request.method), [
    "host.hello",
    "conversation.list",
    "conversation.list",
    "event.subscribe"
  ]);

  harness.sockets[0]?.destroy();
  await waitFor(() => client.connection.phase === "offline");
  assert.equal(scheduler.pending, 1);
  scheduler.runNext();
  await waitFor(() => client.connection.phase === "online" && harness.sockets.length === 2);
  assert.deepEqual(
    client.snapshot.conversations.map((item) => item.title),
    ["Fresh reconnect"]
  );
  assert.deepEqual(harness.requests.slice(-3).map((request) => request.method), [
    "host.hello",
    "conversation.list",
    "event.subscribe"
  ]);

  const liveSocket = harness.sockets[1];
  assert.ok(liveSocket);
  liveSocket.write(
    independentFrame({
      event: "runtime.attached",
      data: {
        attachment: {
          conversationId: conversationId("c"),
          paneId: 7,
          processId: null,
          processOwner: null,
          hostInstanceId: "host-2",
          attachmentGeneration: 1,
          attachedAt: "2026-08-06T19:00:00Z"
        }
      }
    } satisfies HostEvent)
  );
  await waitFor(() => client.snapshot.conversations[0]?.attached === true);
  assert.equal(client.snapshot.conversations[0]?.runtimeState, "unknown_external");
});

void windowsTest("oversized inbound frame closes the real pipe and rejects pending work", async (t) => {
  const harness = await createPipeHarness();
  t.after(async () => harness.close());
  const host = new NamedPipeHost(harness.pipePath);
  t.after(() => host.close());
  await host.open();
  const socket = await harness.nextSocket();
  const disconnected = new Promise<Error>((resolve) => {
    host.onDisconnect(resolve);
  });
  const pending = host.request({ id: 71, method: "host.status" });
  await waitFor(() => harness.requests.some((request) => request.id === 71));

  const invalidHeader = Buffer.alloc(4);
  invalidHeader.writeUInt32LE(MAX_FRAME_BYTES + 1, 0);
  socket.write(invalidHeader);

  await assert.rejects(pending, /outside the allowed/);
  assert.match((await disconnected).message, /outside the allowed/);
});

interface PipeHarness {
  readonly pipePath: string;
  readonly requests: IpcRequest[];
  readonly sockets: Socket[];
  nextSocket(): Promise<Socket>;
  close(): Promise<void>;
}

async function createPipeHarness(
  onRequest?: (socket: Socket, request: IpcRequest, connectionIndex: number) => void
): Promise<PipeHarness> {
  const pipePath = resolvePipePath(`lucidity-at204-${process.pid}-${randomUUID()}`);
  const server = createServer();
  const requests: IpcRequest[] = [];
  const sockets: Socket[] = [];
  server.on("connection", (socket) => {
    sockets.push(socket);
    const connectionIndex = sockets.length;
    observeRequests(socket, (request) => {
      requests.push(request);
      onRequest?.(socket, request, connectionIndex);
    });
  });
  await listen(server, pipePath);
  return {
    pipePath,
    requests,
    sockets,
    async nextSocket() {
      await waitFor(() => sockets.length > 0);
      const socket = sockets.at(-1);
      assert.ok(socket);
      return socket;
    },
    async close() {
      for (const socket of sockets) {
        socket.destroy();
      }
      await closeServer(server);
    }
  };
}

function observeRequests(socket: Socket, onRequest: (request: IpcRequest) => void): void {
  let input: Buffer<ArrayBufferLike> = Buffer.alloc(0);
  socket.on("data", (chunk: Buffer) => {
    input = input.length === 0 ? chunk : Buffer.concat([input, chunk]);
    while (input.length >= 4) {
      const length = input.readUInt32LE(0);
      if (input.length < 4 + length) {
        return;
      }
      const request = JSON.parse(input.subarray(4, 4 + length).toString("utf8")) as IpcRequest;
      input = input.subarray(4 + length);
      onRequest(request);
    }
  });
}

function responseFrame(id: number, payload: HostResult): Buffer {
  return independentFrame({ id, status: "ok", payload } satisfies IpcResponse);
}

function independentFrame(value: unknown): Buffer {
  const body = Buffer.from(JSON.stringify(value), "utf8");
  const header = Buffer.alloc(4);
  header.writeUInt32LE(body.length, 0);
  return Buffer.concat([header, body]);
}

function conversation(suffix: string, title: string): ConversationRecord {
  return {
    id: conversationId(suffix),
    native: {
      adapterId: "fixture-adapter",
      profileId: "22222222-2222-4222-8222-222222222222",
      nativeSessionId: `native-${suffix}`
    },
    nativeSessionPath: null,
    title,
    userAlias: null,
    projectPath: "C:/workspace/Lucidity",
    createdAt: "2026-08-06T18:00:00Z",
    lastActivityAt: "2026-08-06T19:00:00Z",
    organizationState: "active"
  };
}

function conversationId(suffix: string): string {
  const code = suffix.charCodeAt(0).toString(16).padStart(12, "0");
  return `aaaaaaaa-aaaa-4aaa-8aaa-${code}`;
}

function cursor(suffix: string): CatalogCursor {
  return {
    catalogSnapshotVersion: 1,
    lastActivityAt: "2026-08-06T19:00:00Z",
    conversationId: conversationId(suffix)
  };
}

async function listen(server: Server, pipePath: string): Promise<void> {
  await new Promise<void>((resolve, reject) => {
    const onError = (error: Error): void => reject(error);
    server.once("error", onError);
    server.listen(pipePath, () => {
      server.off("error", onError);
      resolve();
    });
  });
}

async function closeServer(server: Server): Promise<void> {
  if (!server.listening) {
    return;
  }
  await new Promise<void>((resolve, reject) => {
    server.close((error) => {
      if (error !== undefined) {
        reject(error);
      } else {
        resolve();
      }
    });
  });
}

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if (predicate()) {
      return;
    }
    await new Promise<void>((resolve) => setTimeout(resolve, 5));
  }
  assert.fail("condition was not reached");
}
