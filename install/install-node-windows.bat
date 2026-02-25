@echo off
:: All-in-one Solana node installer for Windows
:: Version 2.0.0
::
:: Usage:
::   Run from an Administrator Command Prompt, or double-click.
::   For a better experience use install-node-windows.ps1 in PowerShell.
::
:: What this script does:
::   1. Verifies winget is available.
::   2. Installs Git, VS Build Tools (C++ workload), LLVM/Clang, and Node.js via winget.
::   3. Installs Rust via rustup-init.exe.
::   4. Installs the Solana tool suite via the official Windows installer binary.
::   5. Verifies installed tool versions.

setlocal enabledelayedexpansion

set "INSTALLER_VERSION=2.1.0"
set "SOLANA_INSTALLER_URL=https://release.solana.com/stable/solana-install-init-x86_64-pc-windows-msvc.exe"

echo.
echo ============================================================
echo  Solana Node -- Windows Installer v%INSTALLER_VERSION%
echo ============================================================
echo.

:: ---------------------------------------------------------------------------
:: Check for internet connectivity
:: ---------------------------------------------------------------------------
ping -n 1 8.8.8.8 >nul 2>&1
if errorlevel 1 (
    echo [ERROR] No internet connection detected. Please check your network.
    pause
    exit /b 1
)

:: ---------------------------------------------------------------------------
:: 1. Install system dependencies via winget
:: ---------------------------------------------------------------------------
echo [1/5] Checking for winget (Windows Package Manager)...
winget --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] winget is not available on this system.
    echo         Install it from the Microsoft Store ^(App Installer^) or
    echo         update Windows, then re-run this script.
    echo         Alternatively, run install-node-windows.ps1 in PowerShell.
    pause
    exit /b 1
)
echo        winget found.

echo [1/5] Installing Git for Windows...
winget install --id Git.Git -e --silent --accept-source-agreements --accept-package-agreements

echo [1/5] Installing Visual Studio 2022 Build Tools (C++ workload)...
winget install --id Microsoft.VisualStudio.2022.BuildTools -e --silent ^
    --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" ^
    --accept-source-agreements --accept-package-agreements

echo [1/5] Installing LLVM / Clang...
winget install --id LLVM.LLVM -e --silent --accept-source-agreements --accept-package-agreements

echo [1/5] Installing Node.js LTS...
winget install --id OpenJS.NodeJS.LTS -e --silent --accept-source-agreements --accept-package-agreements

:: Refresh PATH in this session to pick up newly installed tools
for /f "tokens=*" %%i in ('powershell -NoProfile -Command "[Environment]::GetEnvironmentVariable(\"PATH\",\"Machine\") + \";\" + [Environment]::GetEnvironmentVariable(\"PATH\",\"User\")"') do set "PATH=%%i"

:: ---------------------------------------------------------------------------
:: 2. Install Rust via rustup
:: ---------------------------------------------------------------------------
echo.
echo [2/5] Installing Rust via rustup...

where rustup >nul 2>&1
if not errorlevel 1 (
    echo        rustup already installed -- updating stable toolchain...
    rustup update stable
) else (
    set "RUSTUP_INIT=%TEMP%\rustup-init.exe"
    echo        Downloading rustup-init.exe...
    powershell -NoProfile -Command ^
        "Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile '!RUSTUP_INIT!' -UseBasicParsing"
    "!RUSTUP_INIT!" -y --default-toolchain stable --no-modify-path
    del "!RUSTUP_INIT!"
)

:: Add cargo bin to PATH for this session
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
rustup component add rustfmt

echo        Rust version:
rustc --version

:: ---------------------------------------------------------------------------
:: 3. Install the Solana tool suite
:: ---------------------------------------------------------------------------
echo.
echo [3/5] Installing Solana tool suite...

set "SOLANA_INIT=%TEMP%\solana-install-init.exe"
powershell -NoProfile -Command ^
    "[Net.ServicePointManager]::SecurityProtocol=[Net.SecurityProtocolType]::Tls12; " ^
    "Invoke-WebRequest -Uri '%SOLANA_INSTALLER_URL%' -OutFile '%SOLANA_INIT%' -UseBasicParsing"

if not exist "%SOLANA_INIT%" (
    echo [ERROR] Failed to download Solana installer.
    pause
    exit /b 1
)

"%SOLANA_INIT%"
del "%SOLANA_INIT%"

:: ---------------------------------------------------------------------------
:: 4. Persist PATH and verify
:: ---------------------------------------------------------------------------
echo.
echo [4/5] Updating system PATH...

set "SOLANA_BIN=%USERPROFILE%\.local\share\solana\install\active_release\bin"
set "PATH=%SOLANA_BIN%;%PATH%"

:: Persist the Solana bin dir in the user PATH via PowerShell
powershell -NoProfile -Command ^
    "$p=[Environment]::GetEnvironmentVariable('PATH','User'); " ^
    "if ($p -notlike '*solana*') { " ^
    "  [Environment]::SetEnvironmentVariable('PATH', \"%SOLANA_BIN%;$p\", 'User') " ^
    "}"

:: ---------------------------------------------------------------------------
:: 5. Verification
:: ---------------------------------------------------------------------------
echo.
echo [5/5] Verifying installations...

solana --version 2>nul
if errorlevel 1 (
    echo [WARN] solana not found in PATH -- you may need to restart your terminal.
) else (
    echo        solana OK
)

rustc --version 2>nul && echo        rustc OK

node --version 2>nul
if not errorlevel 1 ( echo        node OK )

echo.
echo ============================================================
echo  Installation complete!  v%INSTALLER_VERSION%
echo ============================================================
echo.
echo  Solana binaries are at: %SOLANA_BIN%
echo.
echo  If solana is not found, restart your terminal/Command Prompt,
echo  or add the path above to your System / User PATH manually:
echo    Settings -^> System -^> About -^> Advanced System Settings
echo    -^> Environment Variables -^> Path -^> Edit
echo.
echo  Quick-start:
echo    solana --version
echo    solana-keygen new
echo    solana config set --url devnet
echo    solana balance
echo.
pause
endlocal
