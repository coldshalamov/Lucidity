import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import {
  IPC_PROTOCOL_MAX_VERSION,
  IPC_PROTOCOL_MIN_VERSION,
  assertNegotiatedHello,
  unwrapResponse
} from "../src/protocol";
import type { HostEvent, HostRequest } from "../src/protocol";

interface AcceptedProtocolFixture {
  hostRequests: HostRequest[];
  hostEvents: HostEvent[];
}

void test("TypeScript protocol spellings are anchored to the accepted Rust golden fixture", () => {
  const fixturePath = path.resolve(
    __dirname,
    "../../../../agent-protocol/tests/fixtures/enum_struct_variants.json"
  );
  const fixture = JSON.parse(readFileSync(fixturePath, "utf8")) as AcceptedProtocolFixture;

  assert.equal(IPC_PROTOCOL_MIN_VERSION, 1);
  assert.equal(IPC_PROTOCOL_MAX_VERSION, 1);
  assert.deepEqual(fixture.hostRequests[0], {
    method: "host.hello",
    params: { clientName: "fixture-client", minVersion: 1, maxVersion: 1 }
  });
  assert.equal(fixture.hostRequests[6]?.method, "conversation.new");
  assert.deepEqual(fixture.hostEvents.map((event) => event.event), [
    "catalog.changed",
    "conversation.changed",
    "runtime.attached",
    "runtime.detached",
    "usage.changed",
    "adapter.changed"
  ]);
  const catalogEvent = fixture.hostEvents[0];
  if (catalogEvent === undefined || catalogEvent.event !== "catalog.changed") {
    assert.fail("accepted fixture does not start with catalog.changed");
  }
  assert.equal(catalogEvent.data.catalogSnapshotVersion, 10);
});

void test("handshake validation and response correlation fail closed", () => {
  assert.deepEqual(
    assertNegotiatedHello({
      kind: "hello",
      data: { negotiatedVersion: 1, hostInstanceId: "host", capabilities: ["events"] }
    }),
    { negotiatedVersion: 1, hostInstanceId: "host", capabilities: ["events"] }
  );
  assert.throws(
    () =>
      assertNegotiatedHello({
        kind: "hello",
        data: { negotiatedVersion: 2, hostInstanceId: "host", capabilities: [] }
      }),
    /unsupported Lucidity protocol version 2/
  );
  assert.throws(
    () => unwrapResponse({ id: 8, status: "ok", payload: { kind: "accepted" } }, 7),
    /did not match/
  );
  assert.throws(
    () =>
      unwrapResponse(
        {
          id: 7,
          status: "error",
          payload: { code: "offline", message: "missing host", resyncRequired: true }
        },
        7
      ),
    /offline: missing host/
  );
});
