#!/usr/bin/env bash

set -euo pipefail

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "Local app installation is supported only on macOS." >&2
  exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SOURCE_APP="${PROJECT_DIR}/target/release/bundle/macos/Ferephono.app"
INSTALL_DIRECTORY="${HOME}/Applications"
INSTALLED_APP="${INSTALL_DIRECTORY}/Ferephono.app"

if pgrep -x ferephono >/dev/null; then
  echo "Quit Ferephono before installing a new local build." >&2
  exit 1
fi

if [[ -z "${APPLE_SIGNING_IDENTITY:-}" ]]; then
  identities=()
  while IFS= read -r identity; do
    identities+=("${identity}")
  done < <(security find-identity -v -p codesigning | sed -n 's/.*"\(.*\)"/\1/p')

  if [[ "${#identities[@]}" -eq 0 ]]; then
    echo "No code-signing identity found." >&2
    echo "Create a free Apple Development identity in Xcode, then run this command again." >&2
    exit 1
  fi
  if [[ "${#identities[@]}" -gt 1 ]]; then
    echo "Multiple code-signing identities found. Set APPLE_SIGNING_IDENTITY explicitly." >&2
    exit 1
  fi

  APPLE_SIGNING_IDENTITY="${identities[0]}"
  export APPLE_SIGNING_IDENTITY
fi

cd "${PROJECT_DIR}"
npm run tauri -- build --bundles app
"${SCRIPT_DIR}/verify_macos_bundle.sh" "${SOURCE_APP}"
codesign --verify --deep --strict --verbose=2 "${SOURCE_APP}"

mkdir -p "${INSTALL_DIRECTORY}"
ditto "${SOURCE_APP}" "${INSTALLED_APP}"
codesign --verify --deep --strict --verbose=2 "${INSTALLED_APP}"

echo "Installed ${INSTALLED_APP}"
open "${INSTALLED_APP}"
