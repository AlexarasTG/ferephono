export async function connectOverlayState(listen, loadStatus, renderState) {
  let receivedState = false;
  const unlisten = await listen("overlay-state-changed", ({ payload }) => {
    receivedState = true;
    renderState(payload);
  });

  try {
    const status = await loadStatus();
    // A queued snapshot must never overwrite a newer dictation event.
    if (!receivedState) renderState(status.state);
  } catch (error) {
    unlisten();
    throw error;
  }
  return unlisten;
}
