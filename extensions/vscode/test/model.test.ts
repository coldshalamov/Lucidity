import assert from "node:assert/strict";
import test from "node:test";
import fixedFrames from "../fixtures/snapshots/fixed-frames.json";
import stateMatrix from "../fixtures/state-matrix.json";
import { FixtureHost } from "../src/fixtureHost";
import {
  RUNTIME_PRESENTATION,
  conversationsForSection,
  toTreeRow,
  usageStatusText,
  usageTooltip
} from "../src/model";
import type { ConversationPresentation, OrganizationState, RuntimeState } from "../src/protocol";

void test("sections preserve host ordering while separating workspace and organization axes", () => {
  const workspace = "C:\\workspace\\Lucidity";
  const snapshot = new FixtureHost(workspace).presentation();

  assert.deepEqual(ids(conversationsForSection(snapshot, "currentWorkspace", workspace)), [
    "11111111-1111-4111-8111-111111111111",
    "44444444-4444-4444-8444-444444444444"
  ]);
  assert.deepEqual(ids(conversationsForSection(snapshot, "otherProjects", workspace)), [
    "55555555-5555-4555-8555-555555555555",
    "66666666-6666-4666-8666-666666666666"
  ]);
  assert.deepEqual(ids(conversationsForSection(snapshot, "settled", workspace)), [
    "77777777-7777-4777-8777-777777777777",
    "88888888-8888-4888-8888-888888888888"
  ]);
});

void test("all sixteen organization/runtime combinations carry text, shape, icon, and accessible names", () => {
  assert.equal(stateMatrix.length, 16);
  const base = new FixtureHost("C:/workspace/Lucidity").presentation().conversations[0];
  if (base === undefined) {
    assert.fail("fixture conversation is missing");
  }
  const seen = new Set<string>();

  for (const state of stateMatrix as Array<{
    organizationState: OrganizationState;
    runtimeState: RuntimeState;
  }>) {
    const conversation: ConversationPresentation = {
      ...base,
      organizationState: state.organizationState,
      runtimeState: state.runtimeState
    };
    const row = toTreeRow(conversation);
    const runtime = RUNTIME_PRESENTATION[state.runtimeState];
    seen.add(`${state.organizationState}/${state.runtimeState}`);
    assert.match(row.description, new RegExp(runtime.token.replace("?", "\\?")));
    assert.match(row.accessibilityLabel, new RegExp(runtime.label));
    assert.match(row.accessibilityLabel, new RegExp(runtime.cap));
    assert.notEqual(row.icon, "");
    assert.notEqual(row.themeColor, "");
  }
  assert.equal(seen.size, 16);
});

void test("quota and conversation context stay distinct in status text and accessible tooltip", () => {
  const usage = new FixtureHost("C:/workspace/Lucidity").presentation().usage;
  assert.equal(usageStatusText(usage, "online"), "$(pulse) Quota 68% · Context 42%");
  assert.match(usageTooltip(usage, "online"), /Account quota: 68%/);
  assert.match(usageTooltip(usage, "online"), /Conversation context: 42%/);
  assert.equal(usageStatusText(usage, "offline"), "$(plug) Lucidity: Offline");
});

void test("fixed-size semantic snapshots remain deterministic for every accepted extension frame", () => {
  const workspace = "C:/workspace/Lucidity";
  const snapshot = new FixtureHost(workspace).presentation();
  for (const fixture of fixedFrames) {
    assert.ok(fixture.width >= 600);
    assert.ok(fixture.height >= 320);
    assert.deepEqual(
      ids(conversationsForSection(snapshot, "currentWorkspace", workspace)),
      fixture.currentWorkspaceIds,
      fixture.frame
    );
    assert.deepEqual(
      ids(conversationsForSection(snapshot, "otherProjects", workspace)),
      fixture.otherProjectIds,
      fixture.frame
    );
    assert.deepEqual(
      ids(conversationsForSection(snapshot, "settled", workspace)),
      fixture.settledIds,
      fixture.frame
    );
    assert.equal(usageStatusText(snapshot.usage, "online"), fixture.status, fixture.frame);
  }
});

function ids(conversations: ConversationPresentation[]): string[] {
  return conversations.map((conversation) => conversation.id);
}
