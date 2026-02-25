#!/usr/bin/env bash
# All-in-one Solana node installer for macOS
#
# Usage:
#   bash install-node-macos.sh [--update]
#
# Installs Homebrew (if missing), system dependencies, Node.js, Rust,
# and the Solana tool suite.
# Supports: Intel (x86_64) and Apple Silicon (arm64 / M1/M2/M3).

set -euo pipefail

INSTALLER_VERSION="2.1.0"
SOLANA_INSTALL_INIT_URL="https://release.solana.com/stable/install"
NODE_MIN_VERSION=20
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

need_cmd() {
    check_cmd "$1" || err "Required command not found: $1"
}

for arg in "$@"; do
    case "$arg" in
        --update) UPDATE_MODE=true ;;
    esac
done

# ---------------------------------------------------------------------------
# Verify we are on macOS
# ---------------------------------------------------------------------------
check_macos() {
    [ "$(uname -s)" = "Darwin" ] || err "This script is for macOS only."

    local arch
    arch="$(uname -m)"
    info "macOS $(sw_vers -productVersion) on $arch"

    if [ "$arch" = "arm64" ]; then
        info "Apple Silicon (arm64) detected."
        # Ensure Homebrew is evaluated from /opt/homebrew on Apple Silicon
        if [ -f /opt/homebrew/bin/brew ]; then
            eval "$(/opt/homebrew/bin/brew shellenv)"
        fi
    else
        info "Intel (x86_64) detected."
    fi
}

# ---------------------------------------------------------------------------
# 1. Install / update Homebrew
# ---------------------------------------------------------------------------
install_homebrew() {
    if check_cmd brew; then
        info "Homebrew already installed — updating..."
        brew update
    else
        info "Installing Homebrew..."
        /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

        # Add brew to PATH for this session (Apple Silicon path)
        if [ -f /opt/homebrew/bin/brew ]; then
            eval "$(/opt/homebrew/bin/brew shellenv)"
        fi
    fi
}

# ---------------------------------------------------------------------------
# 2. Install system dependencies via Homebrew
# ---------------------------------------------------------------------------
install_system_deps() {
    info "Installing system dependencies via Homebrew..."

    brew install \
        curl wget git \
        openssl@3 pkg-config \
        llvm clang-format cmake make \
        protobuf

    # Ensure openssl is on the path for compilation
    local ossl_prefix
    ossl_prefix="$(brew --prefix openssl@3)"
    export OPENSSL_DIR="$ossl_prefix"
    export OPENSSL_ROOT_DIR="$ossl_prefix"

    info "System dependencies installed."
}

# ---------------------------------------------------------------------------
# 3. Install Node.js via nvm (version-managed, avoids Homebrew conflicts)
# ---------------------------------------------------------------------------
install_nodejs() {
    if check_cmd node; then
        local current_major
        current_major="$(node --version | sed 's/v//' | cut -d. -f1)"
        if [ "$current_major" -ge "$NODE_MIN_VERSION" ] 2>/dev/null; then
            warn "Node.js $(node --version) found but reinstalling to ensure nvm manages it."
        else
            warn "Node.js $(node --version) is below v${NODE_MIN_VERSION} — upgrading via nvm."
        fi
    fi

    info "Installing Node.js v${NODE_MIN_VERSION} via nvm..."

    export NVM_DIR="$HOME/.nvm"
    if [ ! -s "$NVM_DIR/nvm.sh" ]; then
        curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.7/install.sh | bash
    fi
    # shellcheck source=/dev/null
    source "$NVM_DIR/nvm.sh"

    nvm install "${NODE_MIN_VERSION}" --lts
    nvm use "${NODE_MIN_VERSION}"
    nvm alias default "${NODE_MIN_VERSION}"

    info "Node.js $(node --version) installed."
}

# ---------------------------------------------------------------------------
# 4. Install Rust via rustup
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
    info "Rust:  $(rustc --version)"
    info "Cargo: $(cargo --version)"
}

# ---------------------------------------------------------------------------
# 5. Install / update the Solana tool suite
# ---------------------------------------------------------------------------
install_solana() {
    if $UPDATE_MODE && check_cmd solana; then
        info "Updating existing Solana installation..."
        solana-install update
    else
        info "Downloading and running solana-install-init..."
        sh -c "$(curl -sSfL "$SOLANA_INSTALL_INIT_URL")"
    fi

    local shell_rc=""
    if [ -n "${ZSH_VERSION:-}" ] || [ "$(basename "${SHELL:-sh}")" = "zsh" ]; then
        shell_rc="$HOME/.zshrc"
    else
        shell_rc="$HOME/.bash_profile"
    fi

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
# 6. Post-install verification
# ---------------------------------------------------------------------------
verify_install() {
    info "Verifying installation..."
    local all_ok=true

    for tool in solana solana-keygen rustc; do
        if check_cmd "$tool"; then
            info "  ${tool}: $($tool --version 2>&1 | head -1)"
        else
            warn "  ${tool}: NOT FOUND — restart your terminal"
            all_ok=false
        fi
    done

    if check_cmd node; then
        info "  node: $(node --version)"
    fi

    if $all_ok; then
        info "All required tools verified successfully."
    else
        warn "Some tools not found. Run: source \$HOME/.cargo/env && restart terminal."
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    need_cmd curl
    check_macos

    info "=== Solana Node — macOS Installer v${INSTALLER_VERSION} ==="
    if $UPDATE_MODE; then
        info "Mode: UPDATE existing installation"
    fi

    install_homebrew
    install_system_deps
    install_nodejs
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
}

main "$@"
