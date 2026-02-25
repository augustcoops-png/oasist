#Requires -Version 5.1
<#
.SYNOPSIS
    All-in-one Solana node installer for Windows (PowerShell edition).

.DESCRIPTION
    Installs Git, Visual Studio Build Tools (C++ workload), LLVM/Clang,
    Node.js LTS, Rust (via rustup), and the Solana CLI tool suite.
    Works on Windows 10 / 11 and Windows Server 2019+.

.PARAMETER Update
    Update an existing Solana installation instead of a fresh install.

.PARAMETER SkipVsBuildTools
    Skip Visual Studio Build Tools installation (use if you already have MSVC).

.EXAMPLE
    .\install-node-windows.ps1
    .\install-node-windows.ps1 -Update
    .\install-node-windows.ps1 -SkipVsBuildTools
#>
[CmdletBinding()]
param(
    [switch]$Update,
    [switch]$SkipVsBuildTools
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$InstallerVersion   = '2.0.0'
$SolanaInstallerUrl = 'https://release.solana.com/stable/solana-install-init-x86_64-pc-windows-msvc.exe'
$RustupInitUrl      = 'https://win.rustup.rs/x86_64'
$SolanaBinDir       = "$env:USERPROFILE\.local\share\solana\install\active_release\bin"

# -----------------------------------------------------------------------
# Helpers
# -----------------------------------------------------------------------
function Write-Info  { param($Msg) Write-Host "`n[solana-install] $Msg" -ForegroundColor Green }
function Write-Warn  { param($Msg) Write-Host "`n[solana-install] WARN: $Msg" -ForegroundColor Yellow }
function Write-Fail  { param($Msg) Write-Host "`n[solana-install] ERROR: $Msg" -ForegroundColor Red; exit 1 }

function Test-Command {
    param([string]$Name)
    $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Invoke-Download {
    param([string]$Uri, [string]$OutFile)
    Write-Info "Downloading: $Uri"
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $Uri -OutFile $OutFile -UseBasicParsing
}

function Add-ToUserPath {
    param([string]$Dir)
    $current = [Environment]::GetEnvironmentVariable('PATH', 'User')
    if ($current -notlike "*$Dir*") {
        [Environment]::SetEnvironmentVariable('PATH', "$Dir;$current", 'User')
        Write-Info "Added to user PATH: $Dir"
    }
    $env:PATH = "$Dir;$env:PATH"
}

function Test-WingetAvailable {
    if (-not (Test-Command 'winget')) {
        Write-Fail (@"
winget (Windows Package Manager) is not available.
Install it via the Microsoft Store (search for 'App Installer') or update Windows,
then re-run this script.
"@)
    }
}

# -----------------------------------------------------------------------
# Step 1: System packages via winget
# -----------------------------------------------------------------------
function Install-SystemDeps {
    Write-Info "=== Step 1/5: System dependencies ==="
    Test-WingetAvailable

    $packages = @(
        @{ Id = 'Git.Git';                   Name = 'Git for Windows'                   },
        @{ Id = 'LLVM.LLVM';                 Name = 'LLVM / Clang'                      },
        @{ Id = 'OpenJS.NodeJS.LTS';         Name = 'Node.js LTS'                       }
    )

    if (-not $SkipVsBuildTools) {
        $packages += @{ Id = 'Microsoft.VisualStudio.2022.BuildTools'; Name = 'VS 2022 Build Tools' }
    }

    foreach ($pkg in $packages) {
        Write-Info "Installing $($pkg.Name)..."
        $args = @(
            'install', '--id', $pkg.Id, '-e', '--silent',
            '--accept-source-agreements', '--accept-package-agreements'
        )
        if ($pkg.Id -eq 'Microsoft.VisualStudio.2022.BuildTools') {
            $args += @('--override',
                '--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended')
        }
        & winget @args
        if ($LASTEXITCODE -notin @(0, -1978335135)) {  # -1978335135 = already installed
            Write-Warn "$($pkg.Name) returned exit code $LASTEXITCODE — continuing anyway."
        }
    }

    # Refresh PATH in this session
    $env:PATH = [Environment]::GetEnvironmentVariable('PATH','Machine') + ';' +
                [Environment]::GetEnvironmentVariable('PATH','User')
}

# -----------------------------------------------------------------------
# Step 2: Rust via rustup
# -----------------------------------------------------------------------
function Install-Rust {
    Write-Info "=== Step 2/5: Rust ==="
    if (Test-Command 'rustup') {
        Write-Info "rustup already installed — updating stable toolchain..."
        & rustup update stable
    } else {
        $rustupInit = "$env:TEMP\rustup-init.exe"
        Invoke-Download -Uri $RustupInitUrl -OutFile $rustupInit
        & $rustupInit -y --default-toolchain stable --no-modify-path
        Remove-Item $rustupInit -Force -ErrorAction SilentlyContinue
    }

    Add-ToUserPath "$env:USERPROFILE\.cargo\bin"

    & rustup component add rustfmt
    Write-Info "Rust: $(& rustc --version)"
    Write-Info "Cargo: $(& cargo --version)"
}

# -----------------------------------------------------------------------
# Step 3: Solana tool suite
# -----------------------------------------------------------------------
function Install-Solana {
    Write-Info "=== Step 3/5: Solana tool suite ==="

    if ($Update -and (Test-Command 'solana-install')) {
        Write-Info "Updating existing Solana installation..."
        & solana-install update
    } else {
        $solanaInit = "$env:TEMP\solana-install-init.exe"
        Invoke-Download -Uri $SolanaInstallerUrl -OutFile $solanaInit
        & $solanaInit
        Remove-Item $solanaInit -Force -ErrorAction SilentlyContinue
    }

    Add-ToUserPath $SolanaBinDir
}

# -----------------------------------------------------------------------
# Step 4: npm global packages (web3.js tooling)
# -----------------------------------------------------------------------
function Install-NpmGlobals {
    Write-Info "=== Step 4/5: npm global packages ==="
    if (Test-Command 'npm') {
        Write-Info "Installing @solana/web3.js CLI helpers..."
        & npm install -g @solana/web3.js --silent 2>$null
        Write-Info "npm globals installed."
    } else {
        Write-Warn "npm not found in PATH — restart your terminal and re-run to install npm globals."
    }
}

# -----------------------------------------------------------------------
# Step 5: Verify
# -----------------------------------------------------------------------
function Invoke-Verify {
    Write-Info "=== Step 5/5: Verification ==="
    $allOk = $true

    foreach ($tool in @('solana', 'solana-keygen', 'rustc', 'cargo')) {
        if (Test-Command $tool) {
            $ver = & $tool --version 2>&1 | Select-Object -First 1
            Write-Info "  $($tool.PadRight(14)): $ver"
        } else {
            Write-Warn "  $($tool.PadRight(14)): NOT FOUND — restart your terminal"
            $allOk = $false
        }
    }

    if (Test-Command 'node') {
        Write-Info "  $('node'.PadRight(14)): $(& node --version)"
    } else {
        Write-Warn "  $('node'.PadRight(14)): NOT FOUND (restart terminal)"
    }

    if ($allOk) {
        Write-Info "All required tools verified successfully."
    } else {
        Write-Warn "Some tools not found. Please restart your terminal and try again."
    }
}

# -----------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------
Write-Host ""
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host "  Solana Node -- Windows PowerShell Installer v$InstallerVersion" -ForegroundColor Cyan
Write-Host "============================================================" -ForegroundColor Cyan
Write-Host ""

if ($Update) { Write-Info "Mode: UPDATE existing installation" }

# Verify internet
Write-Info "Checking internet connectivity..."
try {
    $null = [System.Net.Dns]::GetHostAddresses('release.solana.com')
    Write-Info "Internet connection OK."
} catch {
    Write-Fail "No internet connection or DNS failure. Please check your network."
}

Install-SystemDeps
Install-Rust
Install-Solana
Install-NpmGlobals
Invoke-Verify

Write-Host ""
Write-Host "============================================================" -ForegroundColor Green
Write-Host "  Installation complete!  v$InstallerVersion" -ForegroundColor Green
Write-Host "============================================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Solana binaries: $SolanaBinDir" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Quick-start commands (in a NEW terminal):" -ForegroundColor White
Write-Host "    solana --version"
Write-Host "    solana-keygen new"
Write-Host "    solana config set --url devnet"
Write-Host "    solana balance"
Write-Host ""
