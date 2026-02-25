#!/usr/bin/env bash
# All-in-one Solana node installer for Linux
#
# Usage:
#   bash install-node-linux.sh
#
# Installs Rust, required system packages, and the Solana tool suite via the
# official solana-install-init bootstrap script.

set -euo pipefail

SOLANA_INSTALL_INIT_URL="https://release.solana.com/stable/install"

info() {
    printf '\n\033[1;32m[solana-install]\033[0m %s\n' "$*"
}

err() {
    printf '\n\033[1;31m[solana-install] ERROR:\033[0m %s\n' "$*" >&2
    exit 1
}

need_cmd() {
    command -v "$1" > /dev/null 2>&1 || err "Required command not found: $1"
}

# ---------------------------------------------------------------------------
# 1. Detect package manager and install system dependencies
# ---------------------------------------------------------------------------
install_system_deps() {
    info "Installing system dependencies..."

    if command -v apt-get > /dev/null 2>&1; then
        sudo apt-get update -y
        sudo apt-get install -y \
            curl wget git build-essential \
            libssl-dev libudev-dev pkg-config \
            zlib1g-dev llvm clang cmake make \
            libprotobuf-dev protobuf-compiler
    elif command -v dnf > /dev/null 2>&1; then
        sudo dnf install -y \
            curl wget git gcc gcc-c++ make \
            openssl-devel systemd-devel pkg-config \
            zlib-devel llvm clang cmake \
            protobuf-devel protobuf-compiler perl-core
    elif command -v pacman > /dev/null 2>&1; then
        sudo pacman -Sy --noconfirm \
            curl wget git base-devel \
            openssl libudev0 pkgconf \
            zlib llvm clang cmake \
            protobuf
    else
        info "Unrecognised package manager — skipping system dependency install."
        info "Please ensure the following are installed manually:"
        info "  curl git build-essential libssl-dev libudev-dev pkg-config"
        info "  zlib1g-dev llvm clang cmake make libprotobuf-dev protobuf-compiler"
    fi
}

# ---------------------------------------------------------------------------
# 2. Install Rust via rustup (if not already present)
# ---------------------------------------------------------------------------
install_rust() {
    if command -v rustup > /dev/null 2>&1; then
        info "rustup is already installed — updating..."
        rustup update stable
    else
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    fi

    # Make cargo available in the current shell session
    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"

    rustup component add rustfmt
    info "Rust version: $(rustc --version)"
}

# ---------------------------------------------------------------------------
# 3. Install the Solana tool suite
# ---------------------------------------------------------------------------
install_solana() {
    info "Downloading and running solana-install-init..."
    sh -c "$(curl -sSfL "$SOLANA_INSTALL_INIT_URL")"

    # Persist the PATH update for future shells
    SHELL_RC=""
    if [ -n "${ZSH_VERSION:-}" ] || [ "$(basename "${SHELL:-}")" = "zsh" ]; then
        SHELL_RC="$HOME/.zshrc"
    else
        SHELL_RC="$HOME/.bashrc"
    fi

    if ! grep -q 'solana' "$SHELL_RC" 2>/dev/null; then
        {
            echo ''
            echo '# Solana tool suite'
            echo 'export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"'
        } >> "$SHELL_RC"
        info "PATH entry added to $SHELL_RC"
    fi

    export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    need_cmd curl
    need_cmd uname

    info "=== Solana Node — Linux Installer ==="
    install_system_deps
    install_rust
    install_solana

    info "=== Installation complete! ==="
    info "Run the following to update your current shell session:"
    info "  source \$HOME/.cargo/env"
    info "  export PATH=\"\$HOME/.local/share/solana/install/active_release/bin:\$PATH\""
    info ""
    info "Verify the installation:"
    info "  solana --version"
}

main "$@"
