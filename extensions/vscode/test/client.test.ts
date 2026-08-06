import assert from "node:assert/strict";
import test from "node:test";
import { HostClient } from "../src/client";
import type { Scheduler } from "../src/client";
import { FixtureHost } from "../src/fixtureHost";
import type { DisposableLike } from "../src/protocol";

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

void test("client performs the exact v1 handshake, initial list, and subscription", async () => {
  const host = new FixtureHost("C:/workspace/Lucidity");
  const client = new HostClient(host);

  await client.start();

  assert.equal(client.connection.phase, "online");
  assert.equal(client.snapshot.conversations.length, 6);
  assert.deepEqual(host.requests.slice(0, 3), [
    {
      id: 1,
      method: "host.hello",
      params: { clientName: "lucidity-vscode", minVersion: 1, maxVersion: 1 }
    },
    { id: 2, method: "conversation.list", params: { cursor: null, limit: 256 } },
    { id: 3, method: "event.subscribe" }
  ]);
  client.dispose();
});

void test("incremental events refresh only their typed surface", async () => {
  const host = new FixtureHost("C:/workspace/Lucidity");
  const client = new HostClient(host);
  await client.start();
  const id = "11111111-1111-4111-8111-111111111111";

  host.applyIncrementalPatch(id, { runtimeState: "completed_idle", eventTally: 13 });
  await nextTurn();

  const updated = client.snapshot.conversations.find((conversation) => conversation.id === id);
  assert.equal(updated?.runtimeState, "completed_idle");
  assert.equal(updated?.eventTally, 13);
  assert.deepEqual(host.requests.at(-1), {
    id: 4,
    method: "conversation.get",
    params: { conversationId: id }
  });
  client.dispose();
});

void test("disconnect enters offline guidance and a scheduled reconnect re-handshakes", async () => {
  const scheduler = new ManualScheduler();
  const host = new FixtureHost("C:/workspace/Lucidity");
  const client = new HostClient(host, scheduler, 1);
  await client.start();

  host.setAvailable(false);
  assert.equal(client.connection.phase, "offline");
  assert.match(client.connection.message, /Start Lucidity Agent Terminal/);
  assert.equal(scheduler.pending, 1);

  host.setAvailable(true);
  scheduler.runNext();
  await waitFor(() => client.connection.phase === "online");

  assert.equal(client.connection.phase, "online");
  assert.deepEqual(host.requests.slice(-3).map((request) => request.method), [
    "host.hello",
    "conversation.list",
    "event.subscribe"
  ]);
  client.dispose();
});

void test("initial host failure remains an explicit error with a scheduled recovery", async () => {
  const scheduler = new ManualScheduler();
  const host = new FixtureHost("C:/workspace/Lucidity");
  host.setAvailable(false);
  const client = new HostClient(host, scheduler, 1);

  await assert.rejects(client.start(), /fixture host is unavailable/);
  assert.equal(client.connection.phase, "error");
  assert.match(client.connection.message, /fixture host is unavailable/);
  assert.equal(scheduler.pending, 1);
  client.dispose();
});

async function nextTurn(): Promise<void> {
  await new Promise<void>((resolve) => setImmediate(resolve));
}

async function waitFor(predicate: () => boolean): Promise<void> {
  for (let attempt = 0; attempt < 20; attempt += 1) {
    if (predicate()) {
      return;
    }
    await nextTurn();
  }
  assert.fail("condition was not reached");
}
