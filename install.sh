#!/usr/bin/env sh
# install.sh — one-line installer for epic
# Usage: curl --proto '=https' --tlsv1.2 -LsSf \
#   https://github.com/epicsagas/epic-harness/releases/latest/download/install.sh | sh
set -eu

REPO="epicsagas/epic-harness"
BINARY="epic"
INSTALL_DIR="${INSTALL_DIR:-${HOME}/.local/bin}"

# ── Detect OS and architecture ────────────────────────────────────────────────
os="$(uname -s | tr '[:upper:]' '[:lower:]')"
arch="$(uname -m)"

case "${os}-${arch}" in
    darwin-arm64|darwin-aarch64) target="aarch64-apple-darwin" ;;
    darwin-x86_64|darwin-amd64)  target="x86_64-apple-darwin" ;;
    linux-arm64|linux-aarch64)   target="aarch64-unknown-linux-gnu" ;;
    linux-x86_64|linux-amd64)    target="x86_64-unknown-linux-gnu" ;;
    *) echo "Error: unsupported platform ${os}-${arch}" >&2; exit 1 ;;
esac

# ── Resolve latest version ────────────────────────────────────────────────────
version="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"v\(.*\)".*/\1/')"
if [ -z "${version}" ]; then
    echo "Error: could not determine latest version" >&2
    exit 1
fi

base_url="https://github.com/${REPO}/releases/download/v${version}"
archive="epic-harness-${target}.tar.xz"
url="${base_url}/${archive}"
sha_url="${base_url}/${archive}.sha256"

# ── Download, verify, and install ────────────────────────────────────────────
tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

echo "Installing ${BINARY} v${version} for ${target}..."

curl -fsSL "${url}"     -o "${tmpdir}/${archive}"
curl -fsSL "${sha_url}" -o "${tmpdir}/${archive}.sha256"

# SHA-256 verification
(cd "${tmpdir}" && sha256sum -c "${archive}.sha256" 2>/dev/null \
    || shasum -a 256 -c "${archive}.sha256") \
    || { echo "Error: SHA-256 verification failed" >&2; exit 1; }

tar -xJf "${tmpdir}/${archive}" -C "${tmpdir}"

mkdir -p "${INSTALL_DIR}"
mv "${tmpdir}/epic-harness-${target}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"

# ── Verify ────────────────────────────────────────────────────────────────────
if command -v "${BINARY}" >/dev/null 2>&1; then
    echo "Installed: $(${BINARY} --version 2>&1 || echo "v${version}")"
else
    echo ""
    echo "Add ${INSTALL_DIR} to your PATH:"
    echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
