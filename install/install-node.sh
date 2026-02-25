#!/usr/bin/env bash
# Cross-platform all-in-one Solana node installer
#
# Detects the current operating system / environment and delegates to the
# appropriate platform-specific installer:
#
#   Linux              -> install-node-linux.sh
#   macOS              -> install-node-macos.sh
#   Android / Termux   -> install-node-termux.sh
#   Windows (Git Bash) -> install-node-windows.ps1 / install-node-windows.bat
#
# Usage:
#   bash install-node.sh [--update] [--help] [--version]
#
# The script can also be piped directly from a URL:
#   bash <(curl -sSfL https://.../install/install-node.sh)

set -euo pipefail

INSTALLER_VERSION="2.1.0"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

info() {
    printf '\n\033[1;32m[solana-install]\033[0m %s\n' "$*"
}

warn() {
    printf '\n\033[1;33m[solana-install] WARN:\033[0m %s\n' "$*" >&2
}

err() {
    printf '\n\033[1;31m[solana-install] ERROR:\033[0m %s\n' "$*" >&2
    exit 1
}

usage() {
    cat <<EOF
Usage: bash install-node.sh [OPTIONS]

All-in-one Solana node installer for Linux, macOS, Windows, and Android/Termux.

Options:
  --update      Update an existing Solana installation instead of a fresh install
  --version     Print installer version and exit
  --help        Show this help message and exit

Supported platforms:
  Linux         Debian/Ubuntu (apt), Fedora/RHEL (dnf/yum), openSUSE (zypper),
                Arch (pacman), Alpine (apk)
  macOS         Homebrew required (will be installed automatically if missing)
  Android       Termux environment
  Windows       Git Bash / MSYS2 / Cygwin — launches PowerShell or .bat installer

Examples:
  bash install-node.sh                # Fresh install (auto-detects OS)
  bash install-node.sh --update       # Update existing installation
EOF
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
    local update_mode=false

    for arg in "$@"; do
        case "$arg" in
            --update)  update_mode=true ;;
            --version) echo "solana-node-installer v${INSTALLER_VERSION}"; exit 0 ;;
            --help|-h) usage; exit 0 ;;
            *) warn "Unknown option: $arg (ignored)" ;;
        esac
    done

    platform="$(detect_platform)"
    info "Detected platform: $platform"
    info "Installer version: ${INSTALLER_VERSION}"

    local extra_args=()
    if $update_mode; then extra_args+=("--update"); fi

    case "$platform" in
        linux)
            info "Running Linux installer..."
            bash "${SCRIPT_DIR}/install-node-linux.sh" "${extra_args[@]}"
            ;;
        macos)
            info "Running macOS installer..."
            bash "${SCRIPT_DIR}/install-node-macos.sh" "${extra_args[@]}"
            ;;
        termux)
            info "Running Android/Termux installer..."
            bash "${SCRIPT_DIR}/install-node-termux.sh" "${extra_args[@]}"
            ;;
        windows)
            info "Windows environment detected (Git Bash / MSYS2 / Cygwin)."
            # Prefer PowerShell script; fall back to .bat
            if command -v powershell.exe > /dev/null 2>&1; then
                info "Launching PowerShell installer..."
                powershell.exe -ExecutionPolicy Bypass \
                    -File "$(cygpath -w "${SCRIPT_DIR}/install-node-windows.ps1" 2>/dev/null \
                              || echo "${SCRIPT_DIR}/install-node-windows.ps1")"
            elif command -v cmd.exe > /dev/null 2>&1; then
                info "Launching batch installer..."
                cmd.exe /c "\"${SCRIPT_DIR}/install-node-windows.bat\""
            else
                err "Cannot find powershell.exe or cmd.exe. Run install-node-windows.bat manually."
            fi
            ;;
        *)
            err "Unsupported platform: $unameOut. Please install manually per the README."
            ;;
    esac
}

main "$@"
