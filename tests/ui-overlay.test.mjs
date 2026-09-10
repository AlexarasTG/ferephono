import assert from "node:assert/strict";
import test from "node:test";
import { readFile } from "node:fs/promises";

import { connectOverlayState } from "../ui/dictation-overlay/state-subscription.mjs";
import { inputAmplitude, waveformPath } from "../ui/dictation-overlay/waveform.mjs";

test("both UI windows have permission to subscribe to state events", async () => {
  const capability = JSON.parse(await readFile(new URL("../capabilities/ui-events.json", import.meta.url)));
  for (const window of ["main", "dictation-overlay"]) assert.ok(capability.windows.includes(window));
  assert.ok(capability.permissions.includes("core:event:allow-listen"));
  assert.ok(capability.permissions.includes("core:event:allow-unlisten"));
});

test("overlay loads current state and receives subsequent dictation events", async () => {
  let onState;
  const rendered = [];
  await connectOverlayState(async (_, handler) => {
    onState = handler;
    return () => {};
  }, async () => ({ state: { kind: "idle" } }), (state) => rendered.push(state.kind));
  for (const kind of ["recording", "transcribing", "inserting", "idle"]) onState({ payload: { kind } });
  assert.deepEqual(rendered, ["idle", "recording", "transcribing", "inserting", "idle"]);
});

test("overlay does not overwrite a recording event with its initial idle snapshot", async () => {
  const rendered = [];
  await connectOverlayState(async (_, handler) => {
    handler({ payload: { kind: "recording" } });
    return () => {};
  }, async () => ({ state: { kind: "idle" } }), (state) => rendered.push(state.kind));
  assert.deepEqual(rendered, ["recording"]);
});

test("overlay surfaces denied event access instead of silently initializing", async () => {
  await assert.rejects(connectOverlayState(
    async () => { throw new Error("event.listen not allowed"); },
    async () => assert.fail("initialization must report the denied subscription"),
    () => assert.fail("no state is available"),
  ), /event.listen not allowed/);
});

test("overlay removes its subscription if initial status loading fails", async () => {
  let unsubscribed = false;
  await assert.rejects(connectOverlayState(
    async () => () => { unsubscribed = true; },
    async () => { throw new Error("status unavailable"); },
    () => {},
  ), /status unavailable/);
  assert.equal(unsubscribed, true);
});


test("waveform input rejects invalid values and distinguishes silence from speech", () => {
  for (const rms of [0, -1, NaN, Infinity, 0.0001]) assert.equal(inputAmplitude(rms), 0);
  assert.ok(inputAmplitude(0.02) > inputAmplitude(0.005));
  assert.ok(inputAmplitude(0.1) > inputAmplitude(0.02));
  assert.equal(inputAmplitude(1), 1);
});

test("waveform stays inside the ghost at maximum volume", () => {
  for (const phase of [0, 1, 3, 6]) {
    const points = [...waveformPath(1, phase).matchAll(/[ML](\d+) ([\d.]+)/g)];
    assert.equal(points.length, 55);
    for (const [, x, y] of points) {
      assert.ok(Number(x) >= 23 && Number(x) <= 77);
      assert.ok(Number(y) >= 36 && Number(y) <= 68);
    }
    assert.equal(Number(points[0][2]), 52);
    assert.equal(Number(points.at(-1)[2]), 52);
  }
});

