import { inputAmplitude, waveformPath } from "./waveform.mjs";
import { connectOverlayState } from "./state-subscription.mjs";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const overlay = document.querySelector("#overlay");
const waveform = document.querySelector("#waveform");
const reducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)");
let state = { kind: "idle" };
let targetAmplitude = 0;
let amplitude = 0;
let animationFrame;
let meterTimer;
let generation = 0;

function overlayLabel(state) {
  switch (state.kind) {
    case "recording": return "Listening";
    case "transcribing": return "Transcribing";
    case "inserting": return "Inserting";
    case "error": return state.message;
    default: return "";
  }
}

function renderState(nextState) {
  generation++;
  state = nextState;
  overlay.dataset.state = state.kind;
  document.querySelector("#label").textContent = overlayLabel(state);
  clearTimeout(meterTimer);
  cancelAnimationFrame(animationFrame);
  targetAmplitude = 0;
  amplitude = 0;
  waveform.setAttribute("d", waveformPath(0, 0));
  if (state.kind === "recording") readLevel(generation);
  if (["recording", "transcribing", "inserting"].includes(state.kind)) animate();
}

async function readLevel(expectedGeneration) {
  try {
    const level = await invoke("get_input_level");
    if (generation !== expectedGeneration) return;
    targetAmplitude = inputAmplitude(level);
  } catch {
    targetAmplitude = 0;
  }
  if (generation === expectedGeneration) {
    meterTimer = setTimeout(() => readLevel(expectedGeneration), 50);
  }
}

function animate(time = 0) {
  const processing = state.kind !== "recording";
  const target = processing ? 0.25 : targetAmplitude;
  amplitude += (target - amplitude) * 0.22;
  const phase = reducedMotion.matches ? 0 : time / (processing ? 320 : 110);
  waveform.setAttribute("d", waveformPath(amplitude, phase));
  animationFrame = requestAnimationFrame(animate);
}

try {
  await connectOverlayState(listen, () => invoke("get_status"), renderState);
  await invoke("report_overlay_initialization", { error: null });
} catch (error) {
  console.error("Unable to initialize dictation overlay", error);
  renderState({ kind: "error", message: "Dictation indicator unavailable. Restart the app." });
  await invoke("report_overlay_initialization", { error: String(error) });
}
