#!/usr/bin/env bash
# Verify LTC Wallet release artifacts. Two modes:
#
#   verify-release.sh checksums <artifact> <SHA256SUMS-file>
#       Check a downloaded artifact against the SHA256SUMS-<platform>.txt
#       attached to the GitHub release. Confirms the file you downloaded is
#       the file CI built (integrity, not provenance).
#
#   verify-release.sh source-build [<tag>]
#       Clone this repo (at <tag>, default: current checkout) into a temp dir,
#       build the app from source (cargo fetches the rev-pinned fork
#       dependencies from the manifests/Cargo.lock), and print SHA-256 hashes
#       of the produced bundles for comparison against the published
#       SHA256SUMS.
#
#       NOTE: builds are not yet bit-for-bit reproducible (.dmg/.AppImage
#       containers embed timestamps), so hashes of containers may differ even
#       for identical source. Comparing the app binary inside the bundle
#       (Contents/MacOS/ltc-wallet) is more stable but still depends on
#       toolchain version. A hash mismatch is a reason to investigate, not
#       automatically an indictment; a match is strong evidence.
set -euo pipefail

REPO_URL="https://github.com/IndigoNakamoto/ltc-wallet-mac"

usage() {
  sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'
  exit 1
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1"
  else
    shasum -a 256 "$1"
  fi
}

cmd_checksums() {
  local artifact="${1:-}" sums="${2:-}"
  [ -n "$artifact" ] && [ -n "$sums" ] || usage
  [ -f "$artifact" ] || { echo "no such file: $artifact" >&2; exit 1; }
  [ -f "$sums" ] || { echo "no such file: $sums" >&2; exit 1; }

  local name expected actual
  name="$(basename "$artifact")"
  expected="$(awk -v n="$name" '$2 == n || $2 == "*"n {print $1}' "$sums")"
  if [ -z "$expected" ]; then
    echo "FAIL: $name not listed in $sums" >&2
    exit 1
  fi
  actual="$(sha256 "$artifact" | awk '{print $1}')"
  if [ "$actual" = "$expected" ]; then
    echo "OK: $name matches published checksum ($actual)"
  else
    echo "FAIL: checksum mismatch for $name" >&2
    echo "  expected: $expected" >&2
    echo "  actual:   $actual" >&2
    exit 1
  fi
}

cmd_source_build() {
  local tag="${1:-}"
  local workdir
  workdir="$(mktemp -d /tmp/ltc-wallet-verify.XXXXXX)"
  echo "Building in $workdir"

  if [ -n "$tag" ]; then
    git clone --depth 1 --branch "$tag" "$REPO_URL" "$workdir/ltc-wallet-mac"
  else
    # Use the repo this script lives in (current checkout, including local changes).
    local here
    here="$(cd "$(dirname "$0")/.." && pwd)"
    git clone "$here" "$workdir/ltc-wallet-mac"
  fi

  echo "Fork dependencies are rev-pinned in the manifests and Cargo.lock;"
  echo "cargo fetches them during the build."

  (
    cd "$workdir/ltc-wallet-mac"
    npm ci
    npm run tauri build
  )

  echo
  echo "SHA-256 of built bundles (compare against the release SHA256SUMS):"
  find "$workdir/ltc-wallet-mac" -path '*/release/bundle/*' \
    \( -name '*.dmg' -o -name '*.deb' -o -name '*.AppImage' \) -type f \
    -print0 | while IFS= read -r -d '' f; do
      sha256 "$f"
    done

  echo
  echo "App binary hashes (more stable across container repacks):"
  find "$workdir/ltc-wallet-mac" -path '*/bundle/macos/*.app/Contents/MacOS/*' -type f \
    -print0 2>/dev/null | while IFS= read -r -d '' f; do
      sha256 "$f"
    done || true

  echo
  echo "Build tree kept at $workdir (delete when done)."
}

case "${1:-}" in
  checksums) shift; cmd_checksums "$@" ;;
  source-build) shift; cmd_source_build "$@" ;;
  *) usage ;;
esac
