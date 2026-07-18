#!/bin/sh

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
DEST_DIR="${1:-${PROJECT_DIR}/runtime}"
WORK_DIR="${2:-${PROJECT_DIR}/.runtime-work}"

set -a
. "${SCRIPT_DIR}/runtime.env"
set +a

WHISPER_CPP_DIR="${WORK_DIR}/whisper.cpp"
MODEL_PATH="${DEST_DIR}/ggml-base.en.bin"

mkdir -p "${DEST_DIR}" "${WORK_DIR}"
if [ -d "${WHISPER_CPP_DIR}" ]; then
  echo "Remove ${WHISPER_CPP_DIR} before preparing a fresh runtime." >&2
  exit 1
fi

git clone --branch "${WHISPER_CPP_REF}" --depth 1 \
  "${WHISPER_CPP_REPOSITORY}" "${WHISPER_CPP_DIR}"

cmake \
  -S "${WHISPER_CPP_DIR}" \
  -B "${WHISPER_CPP_DIR}/build" \
  -DBUILD_SHARED_LIBS=OFF \
  -DWHISPER_BUILD_TESTS=OFF \
  -DWHISPER_BUILD_EXAMPLES=ON
cmake --build "${WHISPER_CPP_DIR}/build" --config Release --target whisper-cli

CLI_PATH="${WHISPER_CPP_DIR}/build/bin/whisper-cli"
if [ ! -x "${CLI_PATH}" ]; then
  CLI_PATH="${WHISPER_CPP_DIR}/build/bin/Release/whisper-cli"
fi
if [ ! -x "${CLI_PATH}" ]; then
  echo "Failed to locate the built whisper-cli executable." >&2
  exit 1
fi

curl --fail --location --silent --show-error \
  "${WHISPER_MODEL_URL}" --output "${MODEL_PATH}"
echo "${WHISPER_MODEL_SHA256}  ${MODEL_PATH}" | shasum -a 256 -c -

cp -f "${CLI_PATH}" "${DEST_DIR}/whisper-cli"
chmod +x "${DEST_DIR}/whisper-cli"

if otool -L "${DEST_DIR}/whisper-cli" | rg -q '@rpath/lib(whisper|ggml)'; then
  echo "whisper-cli is not self-contained." >&2
  otool -L "${DEST_DIR}/whisper-cli" >&2
  exit 1
fi

echo "Runtime prepared in ${DEST_DIR}"

