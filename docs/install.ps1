# rustxl Installation Script for Windows
# This script detects your architecture, downloads the appropriate release,
# and installs the xl.exe binary to your system.

# Set error action preference
$ErrorActionPreference = "Stop"

# GitHub release URL
$RELEASE_BASE_URL = "https://github.com/only-using-ai/rustxl/releases/download/latest"

# Colors for output
function Write-ColorOutput($ForegroundColor) {
    $fc = $host.UI.RawUI.ForegroundColor
    $host.UI.RawUI.ForegroundColor = $ForegroundColor
    if ($args) {
        Write-Output $args
    }
    $host.UI.RawUI.ForegroundColor = $fc
}

function Write-Success {
    Write-ColorOutput Green $args
}

function Write-Error {
    Write-ColorOutput Red $args
}

function Write-Warning {
    Write-ColorOutput Yellow $args
}

# Detect architecture
function Get-Architecture {
    $arch = $env:PROCESSOR_ARCHITECTURE
    
    # Check for 64-bit
    if ($arch -eq "AMD64" -or $arch -eq "x86_64") {
        return "x86_64"
    }
    
    # Check ARM64 (Windows 11 on ARM)
    if ($arch -eq "ARM64") {
        Write-Warning "ARM64 Windows is not currently supported. Falling back to x86_64."
        return "x86_64"
    }
    
    return "x86_64"  # Default to x86_64
}

# Get download URL
function Get-DownloadUrl {
    param($Arch)
    
    if ($Arch -eq "x86_64") {
        return "$RELEASE_BASE_URL/xl-windows-x86_64.zip"
    }
    
    return $null
}

# Check if running as administrator
function Test-Administrator {
    $currentUser = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($currentUser)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# Add to PATH
function Add-ToPath {
    param($Path)
    
    $currentPath = [Environment]::GetEnvironmentVariable("Path", "User")
    
    if ($currentPath -notlike "*$Path*") {
        [Environment]::SetEnvironmentVariable(
            "Path",
            "$currentPath;$Path",
            "User"
        )
        Write-Success "Added $Path to your user PATH"
        Write-Warning "You may need to restart your terminal for the PATH changes to take effect."
    } else {
        Write-Success "$Path is already in your PATH"
    }
}

# Main installation function
function Main {
    Write-Success "rustxl Installation Script"
    Write-Output "================================"
    Write-Output ""
    
    # Detect architecture
    $ARCH = Get-Architecture
    Write-Output "Detected Architecture: $ARCH"
    Write-Output ""
    
    # Get download URL
    $DOWNLOAD_URL = Get-DownloadUrl -Arch $ARCH
    
    if (-not $DOWNLOAD_URL) {
        Write-Error "Error: Could not determine download URL"
        exit 1
    }
    
    Write-Output "Download URL: $DOWNLOAD_URL"
    Write-Output ""
    
    # Create temporary directory
    $TEMP_DIR = Join-Path $env:TEMP "rustxl-install-$(New-Guid)"
    New-Item -ItemType Directory -Path $TEMP_DIR -Force | Out-Null
    
    try {
        Write-Output "Downloading rustxl..."
        
        # Download the archive
        $ARCHIVE_PATH = Join-Path $TEMP_DIR "xl-windows-x86_64.zip"
        
        # Use Invoke-WebRequest with progress
        $ProgressPreference = 'SilentlyContinue'
        Invoke-WebRequest -Uri $DOWNLOAD_URL -OutFile $ARCHIVE_PATH -UseBasicParsing
        $ProgressPreference = 'Continue'
        
        if (-not (Test-Path $ARCHIVE_PATH)) {
            Write-Error "Error: Download failed"
            exit 1
        }
        
        Write-Success "Download complete!"
        Write-Output ""
        
        # Extract the archive
        Write-Output "Extracting archive..."
        Expand-Archive -Path $ARCHIVE_PATH -DestinationPath $TEMP_DIR -Force
        
        # Find xl.exe - it might be directly in the archive or in a subdirectory
        $BINARY_PATH = $null

        # Check if xl.exe is directly in the temp directory
        $DIRECT_PATH = Join-Path $TEMP_DIR "xl.exe"
        if (Test-Path $DIRECT_PATH) {
            $BINARY_PATH = $DIRECT_PATH
        } else {
            # Look for it in a subdirectory
            $EXTRACTED_DIR = Get-ChildItem -Path $TEMP_DIR -Directory -Filter "xl-*" | Select-Object -First 1
            if ($EXTRACTED_DIR -and (Test-Path (Join-Path $EXTRACTED_DIR.FullName "xl.exe"))) {
                $BINARY_PATH = Join-Path $EXTRACTED_DIR.FullName "xl.exe"
            }
        }

        if (-not $BINARY_PATH) {
            Write-Error "Error: Could not find xl.exe in archive"
            Write-Output "Archive contents:"
            Get-ChildItem -Path $TEMP_DIR -Recurse | ForEach-Object { Write-Output $_.FullName }
            exit 1
        }

        Write-Success "Extraction complete!"
        Write-Output ""

        # Determine installation location
        $INSTALL_DIR = Join-Path $env:LOCALAPPDATA "rustxl\bin"

        # Create installation directory if it doesn't exist
        if (-not (Test-Path $INSTALL_DIR)) {
            New-Item -ItemType Directory -Path $INSTALL_DIR -Force | Out-Null
        }

        Write-Output "Installing xl.exe to $INSTALL_DIR..."

        # Copy binary
        Copy-Item -Path $BINARY_PATH -Destination (Join-Path $INSTALL_DIR "xl.exe") -Force
        
        Write-Success "Installation successful!"
        Write-Output ""
        Write-Output "The 'xl.exe' binary has been installed to: $INSTALL_DIR"
        Write-Output ""
        
        # Add to PATH
        Write-Output "Adding to PATH..."
        Add-ToPath -Path $INSTALL_DIR
        Write-Output ""
        
        Write-Success "Installation complete!"
        Write-Output ""
        Write-Output "You can now use 'xl' from any terminal."
        Write-Warning "Note: You may need to restart your terminal for the PATH changes to take effect."
        Write-Output ""
        Write-Output "To verify installation, run: xl --help"
        
    } finally {
        # Clean up temporary directory
        if (Test-Path $TEMP_DIR) {
            Remove-Item -Path $TEMP_DIR -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}

# Check PowerShell version
if ($PSVersionTable.PSVersion.Major -lt 5) {
    Write-Error "This script requires PowerShell 5.0 or later."
    Write-Output "Your PowerShell version: $($PSVersionTable.PSVersion)"
    exit 1
}

# Run main function
Main
