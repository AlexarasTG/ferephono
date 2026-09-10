export const STATUS_REFRESH_INTERVAL_MS = 500;

export function createStatusRefresher(loadStatus, renderStatus) {
  let refreshInProgress = false;

  return async function refreshStatus() {
    if (refreshInProgress) return;

    refreshInProgress = true;
    try {
      renderStatus(await loadStatus());
    } finally {
      refreshInProgress = false;
    }
  };
}
