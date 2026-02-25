#!/usr/bin/env bash
# Cross-platform all-in-one Solana node installer
#
# Detects the current operating system / environment and delegates to the
# appropriate platform-specific installer:
#
#   Linux              -> install-node-linux.sh
#   Android / Termux   -> install-node-termux.sh
#   Windows (Git Bash) -> install-node-windows.bat  (opened for the user)
#   macOS              -> uses the same Linux script (Homebrew path)
#
# Usage:
#   bash install-node.sh
#
# The script can also be piped directly from a URL:
#   bash <(curl -sSfL https://.../install/install-node.sh)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

info() {
    printf '\n\033[1;32m[solana-install]\033[0m %s\n' "$*"
}

err() {
    printf '\n\033[1;31m[solana-install] ERROR:\033[0m %s\n' "$*" >&2
    exit 1
}

# ---------------------------------------------------------------------------
# Detect platform
# ---------------------------------------------------------------------------
detect_platform() {
    unameOut="$(uname -s 2>/dev/null || echo "unknown")"

    # Termux sets $PREFIX to its own path and the kernel reports Linux
    if [ -n "${PREFIX:-}" ] && [ -d "${PREFIX}/bin" ] && echo "$PREFIX" | grep -qi termux; then
        echo "termux"
        return
    fi

    case "$unameOut" in
        Linux*)   echo "linux"   ;;
        Darwin*)  echo "macos"   ;;
        CYGWIN*)  echo "windows" ;;
        MINGW*)   echo "windows" ;;
        MSYS*)    echo "windows" ;;
        *)        echo "unknown" ;;
    esac
}

# ---------------------------------------------------------------------------
# Run the appropriate installer
# ---------------------------------------------------------------------------
main() {
    platform="$(detect_platform)"
    info "Detected platform: $platform"

    case "$platform" in
        linux)
            info "Running Linux installer..."
            bash "${SCRIPT_DIR}/install-node-linux.sh"
            ;;
        macos)
            info "macOS detected — using the Linux installer (Homebrew packages may differ)."
            info "If you encounter missing packages, install them with Homebrew:"
            info "  brew install openssl pkg-config protobuf llvm cmake"
            bash "${SCRIPT_DIR}/install-node-linux.sh"
            ;;
        termux)
            info "Running Android/Termux installer..."
            bash "${SCRIPT_DIR}/install-node-termux.sh"
            ;;
        windows)
            info "Windows environment detected (Git Bash / MSYS2 / Cygwin)."
            info "Please open install-node-windows.bat as Administrator:"
            info "  ${SCRIPT_DIR}/install-node-windows.bat"
            # Attempt to launch the batch file directly if we are in a Windows shell
            if command -v cmd.exe > /dev/null 2>&1; then
                cmd.exe /c "\"${SCRIPT_DIR}/install-node-windows.bat\""
            fi
            ;;
        *)
            err "Unsupported platform: $unameOut"
            ;;
    esac
}

main "$@"
