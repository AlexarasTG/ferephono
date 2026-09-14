import { permissionPresentation, statePresentation, filenameMatchingPresentation } from "./presentation.mjs";
import { STATUS_REFRESH_INTERVAL_MS, createStatusRefresher } from "./status-refresh.mjs";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
let currentState = { kind: "idle" };
let currentPermissions;

function renderState() {
  if (!currentPermissions && currentState.kind === "idle") return;
  const { title, detail, tone, canClearError } = statePresentation(currentState, currentPermissions);
  document.querySelector("#status").textContent = title;
  document.querySelector("#status").dataset.tone = tone;
  document.querySelector("#detail").textContent = detail;
  document.querySelector("#detail").hidden = !detail;
  document.querySelector("#clear-error").hidden = !canClearError;
}

function renderPermissions(permissions) {
  currentPermissions = permissions;
  const { ready } = permissionPresentation(permissions);
  document.querySelector("#permissions").hidden = false;
  document.querySelector("#permission-summary").hidden = !ready;
  document.querySelector("#permission-setup").hidden = ready;
  for (const row of document.querySelectorAll("[data-permission]")) {
    const granted = permissions[row.dataset.permission];
    row.dataset.granted = String(granted);
    row.querySelector(".permission-status").textContent = granted ? "Allowed" : "Required";
  }
  renderState();
}

const refresh = createStatusRefresher(
  () => invoke("get_status"),
  (status) => {
    currentState = status.state;
    renderPermissions(status.permissions);
    renderFilenameMatching(status.filenameMatching);
  },
);

async function refreshStatus() {
  try {
    await refresh();
  } catch (error) {
    console.error("Unable to refresh status", error);
  }
}

async function runAction(button, action) {
  const errorMessage = document.querySelector("#action-error");
  errorMessage.hidden = true;
  button.disabled = true;
  try {
    await action();
  } catch (error) {
    errorMessage.textContent = String(error);
    errorMessage.hidden = false;
  } finally {
    button.disabled = false;
  }
}

document.querySelector("#request-permissions").addEventListener("click", ({ currentTarget }) => {
  runAction(currentTarget, async () => renderPermissions(await invoke("request_permissions")));
});
document.querySelector("#clear-error").addEventListener("click", ({ currentTarget }) => {
  runAction(currentTarget, async () => {
    await invoke("clear_error");
    await refreshStatus();
  });
});
document.querySelector("#quit").addEventListener("click", () => invoke("quit"));

listen("app-state-changed", ({ payload }) => {
  currentState = payload;
  renderState();
}).catch((error) => console.error("Unable to subscribe to dictation state", error));
listen("permissions-changed", ({ payload }) => renderPermissions(payload))
  .catch((error) => console.error("Unable to subscribe to permissions", error));
window.addEventListener("focus", refreshStatus);
document.addEventListener("visibilitychange", () => {
  if (!document.hidden) refreshStatus();
});
refreshStatus();
setInterval(refreshStatus, STATUS_REFRESH_INTERVAL_MS);

function renderFilenameMatching(matching) {
  if (!matching) return;
  const slider = document.querySelector("#matching-sensitivity");
  if (slider.dataset.editing !== "true" && slider.dataset.busy !== "true") {
    slider.value = 100 - matching.minimumSimilarityPercent;
    renderMatchingSensitivity();
  }
  document.querySelector("#reset-matching-sensitivity").dataset.defaultPercent = matching.defaultMinimumSimilarityPercent;
  const toggle = document.querySelector("#filename-matching-enabled");
  if (toggle.dataset.busy !== "true") {
    toggle.checked = matching.enabled;
    toggle.disabled = !matching.folder;
  }
  const button = document.querySelector("#choose-matching-folder");
  button.textContent = matching.folder || "Choose folder…";
  button.setAttribute("aria-label", matching.folder ? `Change Code awareness folder: ${matching.folder}` : "Choose Code awareness folder");
  const status = document.querySelector("#filename-matching-status");
  status.textContent = filenameMatchingPresentation(matching);
  status.dataset.error = String(matching.status.kind === "error");
}

document.querySelector("#filename-matching-enabled").addEventListener("change", async ({ currentTarget }) => {
  currentTarget.dataset.busy = "true";
  const enabled = currentTarget.checked;
  await runAction(currentTarget, () => invoke("set_filename_matching_enabled", { enabled }));
  currentTarget.dataset.busy = "false";
  await refreshStatus();
});

document.querySelector("#choose-matching-folder").addEventListener("click", async ({ currentTarget }) => {
  await runAction(currentTarget, () => invoke("choose_matching_folder"));
  await refreshStatus();
});

function renderMatchingSensitivity() {
  const slider = document.querySelector("#matching-sensitivity");
  const label = `${100 - Number(slider.value)}% minimum similarity`;
  document.querySelector("#matching-sensitivity-value").textContent = label;
  slider.setAttribute("aria-valuetext", label);
}

async function saveMatchingSensitivity(percent) {
  const slider = document.querySelector("#matching-sensitivity");
  const reset = document.querySelector("#reset-matching-sensitivity");
  slider.dataset.busy = "true";
  slider.dataset.editing = "false";
  reset.disabled = true;
  await runAction(slider, () => invoke("set_filename_matching_similarity", { minimumSimilarityPercent: percent }));
  slider.dataset.busy = "false";
  reset.disabled = false;
  await refreshStatus();
}

document.querySelector("#matching-sensitivity").addEventListener("input", ({ currentTarget }) => {
  currentTarget.dataset.editing = "true";
  renderMatchingSensitivity();
});
document.querySelector("#matching-sensitivity").addEventListener("change", ({ currentTarget }) => {
  saveMatchingSensitivity(100 - Number(currentTarget.value));
});
document.querySelector("#reset-matching-sensitivity").addEventListener("click", ({ currentTarget }) => {
  const percent = Number(currentTarget.dataset.defaultPercent);
  document.querySelector("#matching-sensitivity").value = 100 - percent;
  renderMatchingSensitivity();
  saveMatchingSensitivity(percent);
});
