import assert from "node:assert/strict";
import test from "node:test";

import {
  permissionPresentation,
  statePresentation,
} from "../ui/status-panel/presentation.mjs";
import { createStatusRefresher } from "../ui/status-panel/status-refresh.mjs";

const expectedTitles = {
  idle: "Ready to dictate",
  recording: "Listening",
  transcribing: "Transcribing",
  inserting: "Inserting",
};

for (const [kind, title] of Object.entries(expectedTitles)) {
  test(`presents the ${kind} state`, () => {
    const presentation = statePresentation({ kind });
    assert.equal(presentation.title, title);
    if (kind === "idle") assert.equal(presentation.detail, "");
    else assert.ok(presentation.detail.length > 0);
    assert.equal(presentation.canClearError, false);
  });
}

test("presents the application error message", () => {
  const presentation = statePresentation({ kind: "error", message: "Microphone failed" });
  assert.equal(presentation.title, "Needs attention");
  assert.equal(presentation.detail, "Microphone failed");
  assert.equal(presentation.canClearError, true);
});

test("requires all permissions before reporting ready", () => {
  assert.equal(
    permissionPresentation({
      microphoneGranted: true,
      inputMonitoringGranted: true,
      accessibilityGranted: true,
    }).ready,
    true,
  );
  for (const missingPermission of [
    "microphoneGranted",
    "inputMonitoringGranted",
    "accessibilityGranted",
  ]) {
    const permissions = {
      microphoneGranted: true,
      inputMonitoringGranted: true,
      accessibilityGranted: true,
    };
    permissions[missingPermission] = false;
    assert.equal(permissionPresentation(permissions).ready, false);
  }
});

test("status refreshes do not overlap", async () => {
  let loadCount = 0;
  let finishLoading;
  const status = { state: { kind: "idle" }, permissions: {} };
  const loading = new Promise((resolve) => {
    finishLoading = () => resolve(status);
  });
  const rendered = [];
  const refresh = createStatusRefresher(
    () => {
      loadCount += 1;
      return loading;
    },
    (value) => rendered.push(value),
  );

  const firstRefresh = refresh();
  await refresh();
  finishLoading();
  await firstRefresh;

  assert.equal(loadCount, 1);
  assert.deepEqual(rendered, [status]);
});


test("idle panel switches from setup to ready when permissions are granted", () => {
  const permissions = { microphoneGranted: true, inputMonitoringGranted: false, accessibilityGranted: true };
  assert.equal(statePresentation({ kind: "idle" }, permissions).title, "Let’s get you set up");
  permissions.inputMonitoringGranted = true;
  assert.equal(statePresentation({ kind: "idle" }, permissions).title, "Ready to dictate");
  permissions.microphoneGranted = false;
  assert.equal(statePresentation({ kind: "idle" }, permissions).title, "Let’s get you set up");
});

test("permission setup does not hide actionable dictation errors", () => {
  const presentation = statePresentation({ kind: "error", message: "Microphone unavailable" }, {});
  assert.equal(presentation.detail, "Microphone unavailable");
  assert.equal(presentation.canClearError, true);
});

test("automatic status refresh recovers after a failed check", async () => {
  let attempts = 0;
  const rendered = [];
  const refresh = createStatusRefresher(async () => {
    if (++attempts === 1) throw new Error("temporarily unavailable");
    return { permissions: { microphoneGranted: true } };
  }, (status) => rendered.push(status));
  await assert.rejects(refresh(), /temporarily unavailable/);
  await refresh();
  assert.equal(rendered.length, 1);
  assert.equal(rendered[0].permissions.microphoneGranted, true);
});
