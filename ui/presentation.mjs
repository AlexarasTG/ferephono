const presentations = {
  idle: ["Idle", "Hold Right Option to start dictating.", "#6b7280"],
  recording: ["Recording", "Release Right Option to transcribe.", "#188038"],
  transcribing: ["Transcribing", "Converting speech to text locally.", "#b06000"],
  inserting: ["Inserting", "Sending dictated text to the active app.", "#b54708"],
  error: ["Error", "Floating Whisprer needs attention.", "#d93025"],
};

export function statePresentation(state) {
  const [title, defaultDetail, color] = presentations[state.kind];
  return {
    title,
    detail: state.kind === "error" ? state.message : defaultDetail,
    color,
    canClearError: state.kind === "error",
  };
}

export function permissionPresentation(permissions) {
  return {
    ready:
      permissions.microphoneGranted &&
      permissions.inputMonitoringGranted &&
      permissions.accessibilityGranted,
  };
}
