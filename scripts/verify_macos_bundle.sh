#!/bin/sh

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
APP_PATH="${1:-${PROJECT_DIR}/target/release/bundle/macos/Ferephono.app}"
CONTENTS="${APP_PATH}/Contents"
RUNTIME="${CONTENTS}/Resources/runtime"

set -a
. "${SCRIPT_DIR}/runtime.env"
set +a

test -x "${CONTENTS}/MacOS/ferephono"
test -x "${RUNTIME}/whisper-cli"
test -f "${RUNTIME}/ggml-base.en.bin"

test "$(/usr/libexec/PlistBuddy -c 'Print :CFBundleIdentifier' "${CONTENTS}/Info.plist")" = \
  "com.alexarasTG.Ferephono"
test -n "$(/usr/libexec/PlistBuddy -c 'Print :NSMicrophoneUsageDescription' "${CONTENTS}/Info.plist")"
echo "${WHISPER_MODEL_SHA256}  ${RUNTIME}/ggml-base.en.bin" | shasum -a 256 -c -
file "${RUNTIME}/whisper-cli" | grep -q 'arm64'

if codesign -dvv "${APP_PATH}" 2>&1 | grep -q 'flags=.*runtime'; then
  codesign -d --xml --entitlements - "${APP_PATH}" 2>/dev/null | \
    plutil -extract 'com\.apple\.security\.device\.audio-input' raw - | grep -q '^true$'
fi

if otool -L "${RUNTIME}/whisper-cli" | grep -Eq '@rpath/lib(whisper|ggml)'; then
  echo "Bundled whisper-cli references unpackaged whisper/ggml libraries." >&2
  exit 1
fi

echo "Verified ${APP_PATH}"
