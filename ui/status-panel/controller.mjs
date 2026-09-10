import { permissionPresentation, statePresentation } from "./presentation.mjs";
import { STATUS_REFRESH_INTERVAL_MS, createStatusRefresher } from "./status-refresh.mjs";

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

function renderState(state) {
  const { title, detail, color, canClearError } = statePresentation(state);
  document.querySelector("#status b").textContent = title;
  document.querySelector("#status").style.color = color;
  document.querySelector("#status-icon").style.color = color;
  document.querySelector("#detail").textContent = detail;
  document.querySelector("#clear-error").hidden = !canClearError;
}

function renderPermissions(permissions) {
  const presentation = permissionPresentation(permissions);
  document.querySelector("#permissions").innerHTML = presentation.ready
    ? '<p class="permission granted">System Access Ready</p>'
    : `
      <p class="permission ${permissions.microphoneGranted ? "granted" : ""}">Microphone</p>
      <p class="permission ${permissions.inputMonitoringGranted ? "granted" : ""}">Input Monitoring</p>
      <p class="permission ${permissions.accessibilityGranted ? "granted" : ""}">Accessibility</p>
      <button id="request-permissions">Request Permissions</button>
    `;
  document
    .querySelector("#request-permissions")
    ?.addEventListener("click", async () => renderPermissions(await invoke("request_permissions")));
}

const refresh = createStatusRefresher(
  () => invoke("get_status"),
  (status) => {
    renderState(status.state);
    renderPermissions(status.permissions);
  },
);

document.querySelector("#refresh").addEventListener("click", async () => {
  renderPermissions(await invoke("refresh_permissions"));
});
document.querySelector("#clear-error").addEventListener("click", async () => {
  await invoke("clear_error");
});
document.querySelector("#quit").addEventListener("click", () => invoke("quit"));

listen("app-state-changed", ({ payload }) => renderState(payload));
listen("permissions-changed", ({ payload }) => renderPermissions(payload));
refresh();
setInterval(refresh, STATUS_REFRESH_INTERVAL_MS);
