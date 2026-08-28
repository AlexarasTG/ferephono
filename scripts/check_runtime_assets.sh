#!/bin/sh

set -eu

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
RUNTIME_DIR="${1:-${PROJECT_DIR}/runtime}"

missing=""
if [ ! -x "${RUNTIME_DIR}/whisper-cli" ]; then
  missing="${missing} whisper-cli"
fi
if [ ! -f "${RUNTIME_DIR}/ggml-base.en.bin" ]; then
  missing="${missing} ggml-base.en.bin"
fi

if [ -n "${missing}" ]; then
  echo "Whisper runtime is incomplete at ${RUNTIME_DIR}." >&2
  echo "Missing:${missing}" >&2
  echo "Run: npm run prepare:runtime" >&2
  exit 1
fi
