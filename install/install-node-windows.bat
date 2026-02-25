@echo off
:: All-in-one Solana node installer for Windows
::
:: Usage:
::   Double-click install-node-windows.bat, or run it from an Administrator
::   Command Prompt / PowerShell session.
::
:: What this script does:
::   1. Checks that winget (Windows Package Manager) is available.
::   2. Installs Git, Visual Studio Build Tools, and LLVM/Clang via winget.
::   3. Installs Rust via rustup-init.exe (downloads from rustup.rs).
::   4. Installs the Solana tool suite via the official PowerShell installer.

setlocal enabledelayedexpansion

echo.
echo ============================================================
echo  Solana Node -- Windows Installer
echo ============================================================
echo.

:: ---------------------------------------------------------------------------
:: Ensure we have an internet connection by pinging a known host
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
echo [1/4] Checking for winget...
winget --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] winget ^(Windows Package Manager^) is not available on this system.
    echo         Please install it from the Microsoft Store or update Windows, then
    echo         re-run this script.
    pause
    exit /b 1
)

echo [1/4] Installing Git for Windows...
winget install --id Git.Git -e --accept-source-agreements --accept-package-agreements

echo [1/4] Installing Visual Studio Build Tools (C++ workload)...
winget install --id Microsoft.VisualStudio.2022.BuildTools -e ^
    --override "--quiet --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" ^
    --accept-source-agreements --accept-package-agreements

echo [1/4] Installing LLVM / Clang...
winget install --id LLVM.LLVM -e --accept-source-agreements --accept-package-agreements

:: ---------------------------------------------------------------------------
:: 2. Install Rust via rustup
:: ---------------------------------------------------------------------------
echo.
echo [2/4] Installing Rust via rustup...

where rustup >nul 2>&1
if not errorlevel 1 (
    echo         rustup is already installed -- updating...
    rustup update stable
) else (
    :: Download rustup-init.exe to a temp file and run it silently
    set "RUSTUP_INIT=%TEMP%\rustup-init.exe"
    powershell -Command ^
        "Invoke-WebRequest -Uri 'https://win.rustup.rs/x86_64' -OutFile '%RUSTUP_INIT%'"
    "%RUSTUP_INIT%" -y --default-toolchain stable
    del "%RUSTUP_INIT%"
)

:: Reload PATH so cargo/rustup are visible in this session
set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
rustup component add rustfmt

echo         Rust version:
rustc --version

:: ---------------------------------------------------------------------------
:: 3. Install the Solana tool suite via PowerShell
:: ---------------------------------------------------------------------------
echo.
echo [3/4] Installing Solana tool suite...

powershell -Command ^
    "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; " ^
    "Invoke-WebRequest -Uri 'https://release.solana.com/stable/install' -OutFile '%TEMP%\solana-install-init.ps1'; " ^
    "& '%TEMP%\solana-install-init.ps1'"

:: ---------------------------------------------------------------------------
:: 4. Verify and display next steps
:: ---------------------------------------------------------------------------
echo.
echo [4/4] Verifying installation...
set "SOLANA_BIN=%USERPROFILE%\.local\share\solana\install\active_release\bin"
set "PATH=%SOLANA_BIN%;%PATH%"

solana --version 2>nul
if errorlevel 1 (
    echo [WARN] solana binary not found in PATH after install.
    echo        You may need to restart your terminal/Command Prompt.
)

echo.
echo ============================================================
echo  Installation complete!
echo ============================================================
echo.
echo  To make Solana available in every new terminal session,
echo  add the following directory to your system PATH:
echo.
echo    %SOLANA_BIN%
echo.
echo  You can do this via:
echo    System Properties -^> Environment Variables -^> Path -^> Edit
echo.
echo  Then verify the install by running:
echo    solana --version
echo.
pause
endlocal
