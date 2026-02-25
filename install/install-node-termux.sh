#!/usr/bin/env bash
# All-in-one Solana node installer for Android (Termux)
#
# Usage (inside a Termux session):
#   bash install-node-termux.sh [--update]
#
# Installs required Termux packages, Node.js, Rust, and the Solana tool suite.
# Note: Full validator support is limited on Android/ARM; the script installs
# the Solana CLI tools which are fully supported.

set -euo pipefail

INSTALLER_VERSION="2.0.0"
SOLANA_INSTALL_INIT_URL="https://release.solana.com/stable/install"
NODE_MIN_VERSION=18
UPDATE_MODE=false

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

check_cmd() {
    command -v "$1" > /dev/null 2>&1
}

for arg in "$@"; do
    case "$arg" in
        --update) UPDATE_MODE=true ;;
    esac
done

# ---------------------------------------------------------------------------
# Verify we are running inside Termux
# ---------------------------------------------------------------------------
check_termux() {
    if [ -z "${PREFIX:-}" ] || [ ! -d "${PREFIX}/bin" ]; then
        err "This script must be run inside a Termux session on Android."
    fi
    info "Termux environment confirmed: PREFIX=$PREFIX"
    info "Architecture: $(uname -m)"
}

# ---------------------------------------------------------------------------
# 1. Install Termux packages
# ---------------------------------------------------------------------------
install_termux_deps() {
    info "Updating package lists..."
    pkg update -y && pkg upgrade -y

    info "Installing required Termux packages..."
    pkg install -y \
        curl wget git \
        build-essential binutils \
        openssl-dev pkg-config \
        clang cmake make \
        protobuf \
        nodejs-lts \
        python \
        unzip zip \
        which

    info "Node.js $(node --version) installed via pkg."
}

# ---------------------------------------------------------------------------
# 2. Optional: grant storage access (non-blocking)
# ---------------------------------------------------------------------------
setup_storage() {
    if check_cmd termux-setup-storage; then
        info "Setting up Termux storage access (optional — press Allow in the dialog)..."
        termux-setup-storage 2>/dev/null || true
    fi
}

# ---------------------------------------------------------------------------
# 3. Install Rust via rustup
# ---------------------------------------------------------------------------
install_rust() {
    if check_cmd rustup; then
        info "rustup already installed — updating to stable..."
        rustup update stable
    else
        info "Installing Rust via rustup..."
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --default-toolchain stable
    fi

    # shellcheck source=/dev/null
    source "$HOME/.cargo/env"

    rustup component add rustfmt
    info "Rust: $(rustc --version)"
    info "Cargo: $(cargo --version)"
}

# ---------------------------------------------------------------------------
# 4. Install / update the Solana tool suite
# ---------------------------------------------------------------------------
install_solana() {
    if $UPDATE_MODE && check_cmd solana; then
        info "Updating existing Solana installation..."
        solana-install update
    else
        info "Downloading and running solana-install-init..."
        sh -c "$(curl -sSfL "$SOLANA_INSTALL_INIT_URL")"
    fi

    local shell_rc="$HOME/.bashrc"
    if ! grep -q 'solana/install/active_release' "$shell_rc" 2>/dev/null; then
        {
            echo ''
            echo '# Solana tool suite'
            echo 'export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"'
        } >> "$shell_rc"
        info "PATH entry added to $shell_rc"
    fi

    export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
}

# ---------------------------------------------------------------------------
# 5. Post-install verification
# ---------------------------------------------------------------------------
verify_install() {
    info "Verifying installation..."
    local all_ok=true

    if check_cmd solana; then
        info "  solana CLI   : $(solana --version)"
    else
        warn "  solana CLI   : NOT FOUND — restart Termux and re-run"
        all_ok=false
    fi

    if check_cmd solana-keygen; then
        info "  solana-keygen: $(solana-keygen --version)"
    else
        warn "  solana-keygen: NOT FOUND in PATH"
        all_ok=false
    fi

    if check_cmd rustc; then
        info "  rustc        : $(rustc --version)"
    else
        warn "  rustc        : NOT FOUND in PATH"
        all_ok=false
    fi

    if check_cmd node; then
        info "  node         : $(node --version)"
    fi

    if $all_ok; then
        info "All required tools verified successfully."
    else
        warn "Some tools were not found. Restart Termux and retry if needed."
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    check_termux

    info "=== Solana Node — Android/Termux Installer v${INSTALLER_VERSION} ==="
    if $UPDATE_MODE; then
        info "Mode: UPDATE existing installation"
    fi

    install_termux_deps
    setup_storage
    install_rust
    install_solana
    verify_install

    info "=== Installation complete! ==="
    info "Reload your shell environment:"
    info "  source \$HOME/.cargo/env"
    info "  export PATH=\"\$HOME/.local/share/solana/install/active_release/bin:\$PATH\""
    info ""
    info "Quick-start commands:"
    info "  solana --version               # verify CLI"
    info "  solana-keygen new              # generate a keypair"
    info "  solana config set --url devnet # point at devnet"
    info "  solana balance                 # check SOL balance"
    info ""
    info "NOTE: Full validator support is limited on Android/ARM."
    info "      All Solana CLI tools and web3.js are fully supported."
}

main "$@"
