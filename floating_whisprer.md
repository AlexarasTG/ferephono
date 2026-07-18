# Floating Whisprer

Implementation tracker for the independent Rust/Tauri experiment. The existing Swift repository is a read-only behavioural reference and must remain unchanged.

## Decisions

- Repository: local-only `whisprer-rs`; no remote yet.
- Product name: Floating Whisprer.
- Target order: macOS, Windows, Linux.
- UI: Tauri 2 tray-only utility with a minimal HTML/CSS popover; no normal application window.
- Architecture: shared Rust core plus explicit platform adapters.
- MVP: copy current Whisprer behaviour only; no new product features.
- Transcription: bundled `whisper-cli` and `base.en` model, invoked as a subprocess.
- Backend: preserve `--no-gpu` for parity and reliability.
- Runtime assets: versions and checksums tracked; generated binaries remain untracked.
- Hotkey: Right Option. The Swift and Rust apps must not run simultaneously.
- Logging: structured lifecycle/debug logs. Transcript content requires an explicit runtime diagnostic flag.
- Development: TDD for portable behaviour; manual OS validation at the milestone boundary.

## Implementation Steps

- [x] Create an independent Git repository.
- [x] Record decisions, steps and acceptance criteria here.
- [x] Scaffold a minimal Tauri 2 tray application.
- [x] Verify the scaffold builds and its automated tests pass.
- [x] Port developer-focused transcript post-processing test-first.
- [x] Implement the application state machine test-first.
- [x] Implement orchestration against fake transcription and insertion adapters.
- [x] Implement macOS microphone and accessibility permission handling.
- [x] Implement macOS Right Option press/release monitoring.
- [x] Implement 16 kHz mono PCM WAV recording.
- [x] Implement cancellation-safe `whisper-cli` subprocess execution and concurrent pipe draining.
- [x] Implement clipboard-preserving text insertion without the known fixed-delay restore race.
- [x] Connect backend state and actions to the tray popover UI.
- [x] Add pinned runtime asset preparation and Tauri bundle configuration.
- [x] Prevent concurrent Floating Whisprer instances.
- [x] Persist debug logs for troubleshooting GUI launches.
- [x] Run all automated checks and reconcile the MVP acceptance criteria.

## macOS MVP Acceptance Criteria

Automated where possible; items explicitly marked manual must not be checked without a real application run.

- [ ] Runs solely from the macOS menu bar. *(manual)*
- [ ] Completes microphone then accessibility onboarding. *(manual)*
- [ ] Refreshes permission state when the app becomes active or the popover opens. *(manual)*
- [ ] Holding Right Option starts recording and releasing it stops recording. *(manual)*
- [x] Bundles `whisper-cli` and `base.en` and works without network access. *(headless integration verified; app run remains manual)*
- [x] Records 16 kHz, mono, 16-bit PCM WAV audio.
- [x] Invokes Whisper with English, no timestamps, minimal output and `--no-gpu`.
- [x] Produces the same developer-focused post-processing results as the Swift app.
- [ ] Inserts dictated text into an IDE, terminal and browser. *(manual)*
- [ ] Restores the previous clipboard contents after insertion. *(integration/manual)*
- [x] Deletes temporary WAV files after success and failure.
- [x] Recovers to a usable state after transcription or insertion errors.
- [ ] Can run repeatedly without sustained CPU or memory growth. *(manual)*
- [x] Does not log transcript content unless the runtime diagnostic flag is enabled.

## Automated Verification

- [x] `cargo test` (21 tests).
- [x] `cargo clippy --all-targets --all-features -- -D warnings`.
- [x] `npm run test:ui` (6 tests).
- [x] Release `.app` build.
- [x] Bundle identity, permission metadata, ARM64 runtime, model checksum and static Whisper/GGML linkage.
- [x] Headless transcription using the CLI and model from inside the built `.app`.
- [x] Replace the crashing macOS `rdev` listener with a native, platform-scoped event tap.
- [ ] Interactive permission, menu bar, hotkey and text insertion checks when a user is present.

## Required Post-processing Examples

- [x] `the recording-implementation.java` -> `the @RecordingImplementation.java`
- [x] `the MD recording schedule.yaml` -> `the @md-recording-schedule.yaml`
- [x] `the project management.md` -> `the @project-management.md`
- [x] `Please update the developer dictionary.swift file` -> `Please update the @DeveloperDictionary.swift file`
- [x] Normal prose without the strict `the` gate remains unchanged.

## Deferred

- Windows support.
- Linux support and external Wayland/X11 testing.
- In-process `whisper.cpp` bindings.
- GPU retry/fallback policy.
- Settings, model selection, auto-update, streaming and custom commands.
