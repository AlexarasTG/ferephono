import assert from "node:assert/strict";
import test from "node:test";

import {
  permissionPresentation,
  statePresentation,
} from "../ui/presentation.mjs";
import { createStatusRefresher } from "../ui/status-refresh.mjs";

const expectedTitles = {
  idle: "Idle",
  recording: "Recording",
  transcribing: "Transcribing",
  inserting: "Inserting",
};

for (const [kind, title] of Object.entries(expectedTitles)) {
  test(`presents the ${kind} state`, () => {
    const presentation = statePresentation({ kind });
    assert.equal(presentation.title, title);
    assert.ok(presentation.detail.length > 0);
    assert.equal(presentation.canClearError, false);
  });
}

test("presents the application error message", () => {
  const presentation = statePresentation({ kind: "error", message: "Microphone failed" });
  assert.equal(presentation.title, "Error");
  assert.equal(presentation.detail, "Microphone failed");
  assert.equal(presentation.canClearError, true);
});

test("requires both permissions before reporting ready", () => {
  assert.equal(
    permissionPresentation({ microphoneGranted: true, accessibilityGranted: true }).ready,
    true,
  );
  assert.equal(
    permissionPresentation({ microphoneGranted: true, accessibilityGranted: false }).ready,
    false,
  );
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
