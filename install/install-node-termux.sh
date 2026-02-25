#!/usr/bin/env bash
# All-in-one Solana node installer for Android (Termux)
#
# Usage (inside a Termux session):
#   bash install-node-termux.sh
#
# Installs required Termux packages, Rust, and the Solana tool suite.
# Note: Full validator support is limited on Android/ARM; the script installs
# the Solana CLI tools which are fully supported.

set -euo pipefail

SOLANA_INSTALL_INIT_URL="https://release.solana.com/stable/install"

info() {
    printf '\n\033[1;32m[solana-install]\033[0m %s\n' "$*"
}

err() {
    printf '\n\033[1;31m[solana-install] ERROR:\033[0m %s\n' "$*" >&2
    exit 1
}

# ---------------------------------------------------------------------------
# Verify we are running inside Termux
# ---------------------------------------------------------------------------
check_termux() {
    if [ -z "${PREFIX:-}" ] || [ ! -d "${PREFIX}/bin" ]; then
        err "This script must be run inside a Termux session on Android."
    fi
}

# ---------------------------------------------------------------------------
# 1. Install Termux packages
# ---------------------------------------------------------------------------
install_termux_deps() {
    info "Updating package lists..."
    pkg update -y

    info "Installing required Termux packages..."
    pkg install -y \
        curl wget git \
        build-essential binutils \
        openssl-dev pkg-config \
        clang cmake make \
        protobuf
}

# ---------------------------------------------------------------------------
# 2. Install Rust via rustup
# ---------------------------------------------------------------------------
install_rust() {
    if command -v rustup > /dev/null 2>&1; then
        info "rustup is already installed — updating..."
        rustup update stable
    else
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --default-toolchain stable
    fi

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

    SHELL_RC="$HOME/.bashrc"
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
    check_termux

    info "=== Solana Node — Android/Termux Installer ==="
    install_termux_deps
    install_rust
    install_solana

    info "=== Installation complete! ==="
    info "Run the following to update your current shell session:"
    info "  source \$HOME/.cargo/env"
    info "  export PATH=\"\$HOME/.local/share/solana/install/active_release/bin:\$PATH\""
    info ""
    info "Verify the installation:"
    info "  solana --version"
    info ""
    info "NOTE: Full validator/mining support is limited on Android/ARM."
    info "      The Solana CLI tools are fully supported."
}

main "$@"
