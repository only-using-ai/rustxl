@echo off
REM rustxl Installation Script for Windows (Batch)
REM This script detects your architecture, downloads the appropriate release,
REM and installs the xl.exe binary to your system.

setlocal enabledelayedexpansion

echo rustxl Installation Script
echo ================================
echo.

REM Detect architecture (Windows batch is limited, default to x86_64)
set ARCH=x86_64
echo Detected Architecture: %ARCH%
echo.

REM GitHub release URL
set RELEASE_BASE_URL=https://github.com/only-using-ai/rustxl/releases/download/latest
set DOWNLOAD_URL=%RELEASE_BASE_URL%/xl-windows-x86_64.zip

echo Download URL: %DOWNLOAD_URL%
echo.

REM Create temporary directory
set TEMP_DIR=%TEMP%\rustxl-install-%RANDOM%
mkdir "%TEMP_DIR%" 2>nul

echo Downloading rustxl...

REM Check if PowerShell is available (preferred method)
where powershell >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    powershell -NoProfile -ExecutionPolicy Bypass -Command "Invoke-WebRequest -Uri '%DOWNLOAD_URL%' -OutFile '%TEMP_DIR%\xl-windows-x86_64.zip' -UseBasicParsing"
    if %ERRORLEVEL% NEQ 0 (
        echo Error: Download failed
        rmdir /s /q "%TEMP_DIR%" 2>nul
        exit /b 1
    )
) else (
    REM Fallback to certutil (built into Windows)
    certutil -urlcache -split -f "%DOWNLOAD_URL%" "%TEMP_DIR%\xl-windows-x86_64.zip" >nul 2>&1
    if %ERRORLEVEL% NEQ 0 (
        echo Error: Download failed. Please ensure you have internet connectivity.
        rmdir /s /q "%TEMP_DIR%" 2>nul
        exit /b 1
    )
)

echo Download complete!
echo.

echo Extracting archive...

REM Extract using PowerShell (built into Windows 7+)
powershell -NoProfile -ExecutionPolicy Bypass -Command "Expand-Archive -Path '%TEMP_DIR%\xl-windows-x86_64.zip' -DestinationPath '%TEMP_DIR%' -Force" >nul 2>&1

if not exist "%TEMP_DIR%\xl-windows-x86_64\xl.exe" (
    echo Error: Could not find xl.exe in archive
    rmdir /s /q "%TEMP_DIR%" 2>nul
    exit /b 1
)

echo Extraction complete!
echo.

REM Determine installation location
set INSTALL_DIR=%LOCALAPPDATA%\rustxl\bin

REM Create installation directory if it doesn't exist
if not exist "%INSTALL_DIR%" (
    mkdir "%INSTALL_DIR%" 2>nul
)

echo Installing xl.exe to %INSTALL_DIR%...

REM Copy binary
copy /Y "%TEMP_DIR%\xl-windows-x86_64\xl.exe" "%INSTALL_DIR%\xl.exe" >nul 2>&1

if %ERRORLEVEL% NEQ 0 (
    echo Error: Failed to copy xl.exe
    rmdir /s /q "%TEMP_DIR%" 2>nul
    exit /b 1
)

echo Installation successful!
echo.
echo The 'xl.exe' binary has been installed to: %INSTALL_DIR%
echo.

REM Add to PATH
echo Adding to PATH...

REM Get current user PATH
for /f "tokens=2*" %%A in ('reg query "HKCU\Environment" /v Path 2^>nul') do set "CURRENT_PATH=%%B"

REM Check if already in PATH
echo %CURRENT_PATH% | findstr /C:"%INSTALL_DIR%" >nul 2>&1
if %ERRORLEVEL% EQU 0 (
    echo %INSTALL_DIR% is already in your PATH
) else (
    REM Add to PATH
    setx PATH "%CURRENT_PATH%;%INSTALL_DIR%" >nul 2>&1
    if %ERRORLEVEL% EQU 0 (
        echo Added %INSTALL_DIR% to your user PATH
        echo.
        echo Note: You may need to restart your terminal for the PATH changes to take effect.
    ) else (
        echo Warning: Could not automatically add to PATH. Please add %INSTALL_DIR% manually.
    )
)

echo.
echo Installation complete!
echo.
echo You can now use 'xl' from any terminal.
echo Note: You may need to restart your terminal for the PATH changes to take effect.
echo.
echo To verify installation, run: xl --help

REM Clean up temporary directory
rmdir /s /q "%TEMP_DIR%" 2>nul

endlocal
