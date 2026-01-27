#!/bin/bash

# rustxl Installation Script for Linux/macOS
# This script detects your OS and architecture, downloads the appropriate release,
# and installs the xl binary to your system.

set -e  # Exit on error

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# GitHub release URL
RELEASE_BASE_URL="https://github.com/only-using-ai/rustxl/releases/download/latest"

# Detect OS
detect_os() {
    case "$(uname -s)" in
        Linux*)
            echo "linux"
            ;;
        Darwin*)
            echo "macos"
            ;;
        *)
            echo "unknown"
            ;;
    esac
}

# Detect architecture
detect_arch() {
    local arch=$(uname -m)
    case "$arch" in
        x86_64|amd64)
            echo "x86_64"
            ;;
        aarch64|arm64)
            echo "aarch64"
            ;;
        *)
            echo "unknown"
            ;;
    esac
}

# Determine download URL
get_download_url() {
    local os=$1
    local arch=$2
    
    if [ "$os" = "linux" ]; then
        if [ "$arch" = "x86_64" ]; then
            echo "${RELEASE_BASE_URL}/xl-linux-x86_64.tar.gz"
        elif [ "$arch" = "aarch64" ]; then
            echo "${RELEASE_BASE_URL}/xl-linux-aarch64.tar.gz"
        else
            echo "unknown"
        fi
    elif [ "$os" = "macos" ]; then
        if [ "$arch" = "x86_64" ]; then
            echo "${RELEASE_BASE_URL}/xl-macos-x86_64.tar.gz"
        elif [ "$arch" = "aarch64" ]; then
            echo "${RELEASE_BASE_URL}/xl-macos-arm64.tar.gz"
        else
            echo "unknown"
        fi
    else
        echo "unknown"
    fi
}

# Get archive name
get_archive_name() {
    local os=$1
    local arch=$2
    
    if [ "$os" = "linux" ]; then
        if [ "$arch" = "x86_64" ]; then
            echo "xl-linux-x86_64.tar.gz"
        elif [ "$arch" = "aarch64" ]; then
            echo "xl-linux-aarch64.tar.gz"
        fi
    elif [ "$os" = "macos" ]; then
        if [ "$arch" = "x86_64" ]; then
            echo "xl-macos-x86_64.tar.gz"
        elif [ "$arch" = "aarch64" ]; then
            echo "xl-macos-arm64.tar.gz"
        fi
    fi
}

# Check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Main installation function
main() {
    echo -e "${GREEN}rustxl Installation Script${NC}"
    echo "================================"
    echo ""
    
    # Detect OS and architecture
    OS=$(detect_os)
    ARCH=$(detect_arch)
    
    echo "Detected OS: $OS"
    echo "Detected Architecture: $ARCH"
    echo ""
    
    if [ "$OS" = "unknown" ] || [ "$ARCH" = "unknown" ]; then
        echo -e "${RED}Error: Unsupported OS or architecture${NC}"
        echo "Supported platforms:"
        echo "  - Linux (x86_64, aarch64)"
        echo "  - macOS (x86_64, arm64)"
        exit 1
    fi
    
    # Get download URL
    DOWNLOAD_URL=$(get_download_url "$OS" "$ARCH")
    ARCHIVE_NAME=$(get_archive_name "$OS" "$ARCH")
    
    if [ "$DOWNLOAD_URL" = "unknown" ]; then
        echo -e "${RED}Error: Could not determine download URL${NC}"
        exit 1
    fi
    
    echo "Download URL: $DOWNLOAD_URL"
    echo ""
    
    # Check for required commands
    if ! command_exists curl && ! command_exists wget; then
        echo -e "${RED}Error: Neither curl nor wget is installed${NC}"
        echo "Please install one of them and try again."
        exit 1
    fi
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    trap "rm -rf $TEMP_DIR" EXIT
    
    echo "Downloading rustxl..."
    
    # Download the archive
    if command_exists curl; then
        curl -L -o "$TEMP_DIR/$ARCHIVE_NAME" "$DOWNLOAD_URL"
    else
        wget -O "$TEMP_DIR/$ARCHIVE_NAME" "$DOWNLOAD_URL"
    fi
    
    if [ ! -f "$TEMP_DIR/$ARCHIVE_NAME" ]; then
        echo -e "${RED}Error: Download failed${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Download complete!${NC}"
    echo ""
    
    # Extract the archive
    echo "Extracting archive..."
    cd "$TEMP_DIR"
    tar -xzf "$ARCHIVE_NAME"
    
    # Find the extracted directory
    EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "xl-*" | head -n 1)
    
    if [ -z "$EXTRACTED_DIR" ] || [ ! -f "$EXTRACTED_DIR/xl" ]; then
        echo -e "${RED}Error: Could not find xl binary in archive${NC}"
        exit 1
    fi
    
    echo -e "${GREEN}Extraction complete!${NC}"
    echo ""
    
    # Determine installation location
    if [ -w "/usr/local/bin" ]; then
        INSTALL_DIR="/usr/local/bin"
        USE_SUDO=false
    elif [ -w "$HOME/.local/bin" ]; then
        INSTALL_DIR="$HOME/.local/bin"
        USE_SUDO=false
    else
        INSTALL_DIR="/usr/local/bin"
        USE_SUDO=true
    fi
    
    echo "Installing xl to $INSTALL_DIR..."
    
    # Copy binary
    if [ "$USE_SUDO" = true ]; then
        sudo cp "$EXTRACTED_DIR/xl" "$INSTALL_DIR/xl"
        sudo chmod +x "$INSTALL_DIR/xl"
    else
        cp "$EXTRACTED_DIR/xl" "$INSTALL_DIR/xl"
        chmod +x "$INSTALL_DIR/xl"
    fi
    
    # Check if installation was successful
    if command_exists xl; then
        echo ""
        echo -e "${GREEN}Installation successful!${NC}"
        echo ""
        echo "The 'xl' command is now available."
        echo "You can verify by running: xl --help"
        
        # Check if ~/.local/bin is in PATH
        if [ "$INSTALL_DIR" = "$HOME/.local/bin" ]; then
            if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
                echo ""
                echo -e "${YELLOW}Note: $HOME/.local/bin is not in your PATH.${NC}"
                echo "Add this line to your ~/.bashrc, ~/.zshrc, or ~/.profile:"
                echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
            fi
        fi
    else
        echo ""
        echo -e "${YELLOW}Warning: Installation completed, but 'xl' command not found in PATH.${NC}"
        echo "The binary was installed to: $INSTALL_DIR/xl"
        echo "Make sure $INSTALL_DIR is in your PATH."
    fi
}

# Run main function
main "$@"
