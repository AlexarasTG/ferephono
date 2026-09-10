export function inputAmplitude(rms) {
  if (!Number.isFinite(rms) || rms <= 0) return 0;
  // Map a useful speech range (-54 to -12 dBFS) onto the small waveform.
  return Math.max(0, Math.min(1, (20 * Math.log10(rms) + 54) / 42));
}

export function waveformPath(amplitude, phase) {
  const points = [];
  for (let i = 0; i <= 54; i++) {
    const envelope = Math.sin((i / 54) * Math.PI) ** 2;
    const y = 52 + Math.sin(i * 0.48 - phase) * envelope * (0.6 + amplitude * 15);
    points.push(`${i === 0 ? "M" : "L"}${23 + i} ${y.toFixed(2)}`);
  }
  return points.join(" ");
}

export function overlayLabel(state) {
  switch (state.kind) {
    case "recording": return "Listening";
    case "transcribing": return "Transcribing";
    case "inserting": return "Inserting";
    case "error": return state.message;
    default: return "";
  }
}
