# rustxl Installation Scripts

This directory contains installation scripts for rustxl that automatically detect your operating system and architecture, then download and install the appropriate release.

## Available Scripts

### Unix-like Systems (Linux, macOS)
- **install.sh** - Shell script for Linux and macOS

### Windows
- **install.ps1** - PowerShell script (recommended for Windows)
- **install.bat** - Batch script (fallback for older Windows systems)

## Usage

### Linux/macOS

```bash
# Download and run the installation script
curl -fsSL https://raw.githubusercontent.com/only-using-ai/rustxl/main/scripts/install/install.sh | bash

# Or download first, then run
wget https://raw.githubusercontent.com/only-using-ai/rustxl/main/scripts/install/install.sh
chmod +x install.sh
./install.sh
```

### Windows (PowerShell - Recommended)

```powershell
# Run directly from the web
Invoke-WebRequest -Uri https://raw.githubusercontent.com/only-using-ai/rustxl/main/scripts/install/install.ps1 -OutFile install.ps1
.\install.ps1

# Or if you've cloned the repository
.\scripts\install\install.ps1
```

**Note:** If you encounter an execution policy error, run:
```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
```

### Windows (Batch)

```cmd
# Download and run
powershell -Command "Invoke-WebRequest -Uri https://raw.githubusercontent.com/only-using-ai/rustxl/main/scripts/install/install.bat -OutFile install.bat"
install.bat

# Or if you've cloned the repository
scripts\install\install.bat
```

## What the Scripts Do

1. **Detect your system**: Automatically detects your OS (Linux, macOS, Windows) and architecture (x86_64, aarch64/arm64)
2. **Download the correct release**: Downloads the appropriate pre-built binary from GitHub releases
3. **Extract the archive**: Extracts the downloaded archive
4. **Install the binary**: 
   - **Linux/macOS**: Installs `xl` to `/usr/local/bin` (if writable) or `~/.local/bin`
   - **Windows**: Installs `xl.exe` to `%LOCALAPPDATA%\rustxl\bin`
5. **Update PATH**: Automatically adds the installation directory to your PATH (Windows) or provides instructions (Linux/macOS)

## Supported Platforms

- **Linux**: x86_64, aarch64
- **macOS**: x86_64 (Intel), arm64 (Apple Silicon)
- **Windows**: x86_64

## Troubleshooting

### Linux/macOS

- If installation fails due to permissions, the script will use `sudo` for `/usr/local/bin`
- If `~/.local/bin` is not in your PATH, add this to your `~/.bashrc`, `~/.zshrc`, or `~/.profile`:
  ```bash
  export PATH="$HOME/.local/bin:$PATH"
  ```

### Windows

- If PowerShell execution is blocked, run:
  ```powershell
  Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
  ```
- After installation, restart your terminal for PATH changes to take effect
- If `xl` command is not found, verify that `%LOCALAPPDATA%\rustxl\bin` is in your PATH

## Manual Installation

If you prefer to install manually:

1. Download the appropriate archive from [GitHub Releases](https://github.com/only-using-ai/rustxl/releases)
2. Extract the archive
3. Move the binary (`xl` or `xl.exe`) to a directory in your PATH
4. Make it executable (Unix-like systems): `chmod +x xl`
