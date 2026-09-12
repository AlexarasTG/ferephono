const presentations = {
  idle: ["Ready to dictate", "", "ready"],
  recording: ["Listening", "Release Right Option when you’re done.", "active"],
  transcribing: ["Transcribing", "Turning your speech into text locally.", "active"],
  inserting: ["Inserting", "Sending your words to the active app.", "active"],
  error: ["Needs attention", "Floating Whisprer needs attention.", "error"],
};

export function statePresentation(state, permissions) {
  const [title, defaultDetail, tone] = presentations[state.kind];
  if (state.kind === "idle" && permissions && !permissionPresentation(permissions).ready) {
    return {
      title: "Let’s get you set up",
      detail: "Allow the system access below to start dictating.",
      tone: "muted",
      canClearError: false,
    };
  }
  return {
    title,
    detail: state.kind === "error" ? state.message : defaultDetail,
    tone,
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
