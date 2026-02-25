#!/usr/bin/env bash
# All-in-one Solana node installer for Linux
#
# Usage:
#   bash install-node-linux.sh [--update]
#
# Installs Rust, required system packages, Node.js, and the Solana tool suite.
# Supports: Debian/Ubuntu (apt), Fedora/RHEL (dnf), CentOS/RHEL 7 (yum),
#           openSUSE (zypper), Arch Linux (pacman), Alpine Linux (apk)

set -euo pipefail

INSTALLER_VERSION="2.1.0"
SOLANA_INSTALL_INIT_URL="https://release.solana.com/stable/install"
# Node.js Active LTS version for web3.js / tooling
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

need_cmd() {
    command -v "$1" > /dev/null 2>&1 || err "Required command not found: $1"
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
# 1. Detect package manager and install system dependencies
# ---------------------------------------------------------------------------
install_system_deps() {
    info "Installing system dependencies..."

    local arch
    arch="$(uname -m)"
    info "Architecture: $arch"

    if check_cmd apt-get; then
        sudo apt-get update -y
        sudo apt-get install -y \
            curl wget git build-essential \
            libssl-dev libudev-dev pkg-config \
            zlib1g-dev llvm clang cmake make \
            libprotobuf-dev protobuf-compiler \
            ca-certificates gnupg lsb-release

    elif check_cmd dnf; then
        sudo dnf install -y \
            curl wget git gcc gcc-c++ make \
            openssl-devel systemd-devel pkg-config \
            zlib-devel llvm clang cmake \
            protobuf-devel protobuf-compiler perl-core \
            ca-certificates

    elif check_cmd yum; then
        # CentOS / RHEL 7
        sudo yum groupinstall -y "Development Tools"
        sudo yum install -y \
            curl wget git \
            openssl-devel systemd-devel pkgconfig \
            zlib-devel llvm clang cmake \
            ca-certificates

    elif check_cmd zypper; then
        # openSUSE / SUSE
        sudo zypper --non-interactive install \
            curl wget git gcc gcc-c++ make \
            libopenssl-devel systemd-devel pkg-config \
            zlib-devel llvm clang cmake \
            protobuf-devel ca-certificates

    elif check_cmd pacman; then
        # Arch Linux
        sudo pacman -Sy --noconfirm \
            curl wget git base-devel \
            openssl libudev0 pkgconf \
            zlib llvm clang cmake \
            protobuf ca-certificates

    elif check_cmd apk; then
        # Alpine Linux
        sudo apk add --no-cache \
            curl wget git build-base \
            openssl-dev linux-headers pkgconf \
            zlib-dev llvm clang cmake make \
            protobuf-dev ca-certificates

    else
        warn "Unrecognised package manager — skipping system dependency install."
        warn "Please install manually: curl git build-essential libssl-dev libudev-dev"
        warn "pkg-config zlib1g-dev llvm clang cmake make libprotobuf-dev protobuf-compiler"
    fi
}

# ---------------------------------------------------------------------------
# 2. Install Node.js (for Solana web3.js tooling and the web ledger)
# ---------------------------------------------------------------------------
install_nodejs() {
    # Always install/update Node.js via nvm — never skip
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

    # Persist the PATH update for future shells
    local shell_rc=""
    if [ -n "${ZSH_VERSION:-}" ] || [ "$(basename "${SHELL:-sh}")" = "zsh" ]; then
        shell_rc="$HOME/.zshrc"
    else
        shell_rc="$HOME/.bashrc"
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
# 5. Post-install verification
# ---------------------------------------------------------------------------
verify_install() {
    info "Verifying installation..."
    local all_ok=true

    if check_cmd solana; then
        info "  solana CLI  : $(solana --version)"
    else
        warn "  solana CLI  : NOT FOUND in PATH"
        all_ok=false
    fi

    if check_cmd solana-keygen; then
        info "  solana-keygen: $(solana-keygen --version)"
    else
        warn "  solana-keygen: NOT FOUND in PATH"
        all_ok=false
    fi

    if check_cmd rustc; then
        info "  rustc       : $(rustc --version)"
    else
        warn "  rustc       : NOT FOUND in PATH"
        all_ok=false
    fi

    if check_cmd node; then
        info "  node        : $(node --version)"
    else
        warn "  node        : NOT FOUND (optional)"
    fi

    if $all_ok; then
        info "All required tools verified successfully."
    else
        warn "Some tools were not found. You may need to restart your terminal."
    fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
main() {
    need_cmd curl
    need_cmd uname

    info "=== Solana Node — Linux Installer v${INSTALLER_VERSION} ==="
    if $UPDATE_MODE; then
        info "Mode: UPDATE existing installation"
    fi

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
    info "  solana --version          # verify CLI"
    info "  solana-keygen new         # generate a keypair"
    info "  solana config set --url devnet  # point at devnet"
    info "  solana balance            # check SOL balance"
}

main "$@"
