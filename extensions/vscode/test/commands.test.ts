import assert from "node:assert/strict";
import test from "node:test";
import { createCommandHandlers } from "../src/commands";
import type { HostCommandClient } from "../src/commands";
import { emptyPresentation } from "../src/model";
import type { HostRequest, HostResult } from "../src/protocol";

class RecordingClient implements HostCommandClient {
  public readonly requests: HostRequest[] = [];
  public readonly snapshot = emptyPresentation();
  public reconnectCount = 0;

  public async request(request: HostRequest): Promise<HostResult> {
    this.requests.push(request);
    return { kind: "accepted" };
  }

  public async reconnectNow(): Promise<void> {
    this.reconnectCount += 1;
  }
}

void test("every command emits the frozen method and exact current-workspace payload", async () => {
  const client = new RecordingClient();
  client.snapshot.usage.conversationId = "11111111-1111-4111-8111-111111111111";
  const information: string[] = [];
  const handlers = createCommandHandlers(client, {
    workspacePath: () => "C:\\workspace\\Lucidity",
    adapterId: () => "fixture-adapter",
    profileId: () => "22222222-2222-4222-8222-222222222222",
    confirmStop: async () => true,
    inform: (message) => information.push(message)
  });
  const target = { id: "11111111-1111-4111-8111-111111111111", label: "Fixture" };

  await handlers.newInCurrentWorkspace();
  await handlers.openFocus(target);
  await handlers.settle(target);
  await handlers.unsettle(target);
  await handlers.stop(target);
  await handlers.refreshUsage();
  await handlers.openLucidity();
  await handlers.reconnect();

  assert.deepEqual(client.requests, [
    {
      method: "conversation.new",
      params: {
        adapterId: "fixture-adapter",
        profileId: "22222222-2222-4222-8222-222222222222",
        projectPath: "C:\\workspace\\Lucidity"
      }
    },
    {
      method: "conversation.open",
      params: { conversationId: "11111111-1111-4111-8111-111111111111" }
    },
    {
      method: "conversation.setOrganizationState",
      params: {
        conversationId: "11111111-1111-4111-8111-111111111111",
        organizationState: "settled"
      }
    },
    {
      method: "conversation.setOrganizationState",
      params: {
        conversationId: "11111111-1111-4111-8111-111111111111",
        organizationState: "active"
      }
    },
    {
      method: "conversation.stop",
      params: { conversationId: "11111111-1111-4111-8111-111111111111" }
    },
    {
      method: "conversation.refreshUsage",
      params: { conversationId: "11111111-1111-4111-8111-111111111111" }
    },
    { method: "host.openUi" }
  ]);
  assert.equal(client.reconnectCount, 1);
  assert.deepEqual(information, []);
});

void test("stop requires confirmation and missing usage stays explicit", async () => {
  const client = new RecordingClient();
  const information: string[] = [];
  const handlers = createCommandHandlers(client, {
    workspacePath: () => "C:/workspace/Lucidity",
    adapterId: () => "fixture-adapter",
    profileId: () => "profile",
    confirmStop: async () => false,
    inform: (message) => information.push(message)
  });

  await handlers.stop({ id: "conversation", label: "Fixture" });
  await handlers.refreshUsage();
  assert.deepEqual(client.requests, []);
  assert.deepEqual(information, ["No attached conversation has usage data to refresh."]);
});

void test("new command refuses to invent a workspace payload", async () => {
  const client = new RecordingClient();
  const handlers = createCommandHandlers(client, {
    workspacePath: () => undefined,
    adapterId: () => "fixture-adapter",
    profileId: () => "profile",
    confirmStop: async () => true,
    inform: () => undefined
  });

  await assert.rejects(handlers.newInCurrentWorkspace(), /Open a workspace folder/);
  assert.deepEqual(client.requests, []);
});
