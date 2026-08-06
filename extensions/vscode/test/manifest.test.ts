import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import path from "node:path";
import test from "node:test";
import manifest from "../package.json";

void test("manifest is lazy, native-only, and contributes the complete command/view surface", () => {
  assert.equal(manifest.activationEvents.includes("*"), false);
  assert.deepEqual(
    manifest.contributes.views.lucidity.map((view) => view.name),
    ["Current Workspace", "Other Projects", "Settled"]
  );
  assert.deepEqual(
    manifest.contributes.commands.map((command) => command.command),
    [
      "lucidity.newInCurrentWorkspace",
      "lucidity.openFocus",
      "lucidity.settle",
      "lucidity.unsettle",
      "lucidity.stop",
      "lucidity.refreshUsage",
      "lucidity.openLucidity",
      "lucidity.reconnect"
    ]
  );
  assert.equal(JSON.stringify(manifest).toLocaleLowerCase("en-US").includes("webview"), false);
  assert.equal(manifest.main, "./dist/src/extension.js");
});

void test("Activity Bar icon preserves the frozen achromatic Weld Frame geometry", () => {
  const icon = readFileSync(path.resolve(__dirname, "../../resources/weld-frame.svg"), "utf8");
  for (const rectangle of [
    'x="2" y="2" width="3" height="12"',
    'x="5" y="2" width="7" height="3"',
    'x="5" y="11" width="7" height="3"',
    'x="7" y="6" width="2" height="4"',
    'x="11" y="6" width="3" height="4"'
  ]) {
    assert.match(icon, new RegExp(rectangle));
  }
  assert.match(icon, /currentColor/);
  assert.doesNotMatch(icon, /#[0-9a-f]{3,8}/i);
});
