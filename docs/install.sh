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
    
    # Download the archive with error checking
    HTTP_CODE=0
    if command_exists curl; then
        # Download and capture HTTP code separately
        HTTP_CODE=$(curl -L -w "%{http_code}" -o "$TEMP_DIR/$ARCHIVE_NAME" -s "$DOWNLOAD_URL" 2>&1 | tail -n1)
        CURL_EXIT=$?
        
        # Check if download was successful
        if [ $CURL_EXIT -ne 0 ] || [ "$HTTP_CODE" != "200" ]; then
            echo -e "${RED}Error: Download failed${NC}"
            if [ -n "$HTTP_CODE" ] && [ "$HTTP_CODE" != "200" ]; then
                echo "HTTP status code: $HTTP_CODE"
            fi
            if [ -f "$TEMP_DIR/$ARCHIVE_NAME" ]; then
                echo ""
                echo "Downloaded content (first 500 chars):"
                head -c 500 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || true
                echo ""
            fi
            exit 1
        fi
    else
        # Use wget with better error handling
        if ! wget -O "$TEMP_DIR/$ARCHIVE_NAME" "$DOWNLOAD_URL" 2>&1; then
            echo -e "${RED}Error: Download failed${NC}"
            if [ -f "$TEMP_DIR/$ARCHIVE_NAME" ]; then
                echo ""
                echo "Downloaded content (first 500 chars):"
                head -c 500 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || true
                echo ""
            fi
            exit 1
        fi
    fi
    
    if [ ! -f "$TEMP_DIR/$ARCHIVE_NAME" ]; then
        echo -e "${RED}Error: Download failed - file not found${NC}"
        exit 1
    fi
    
    # Verify the downloaded file is actually a tar.gz archive
    # Check gzip magic bytes (1f 8b) at the start of the file
    FIRST_BYTES=$(head -c 2 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null | od -An -tx1 | tr -d ' \n' || echo "")
    if [ "$FIRST_BYTES" != "1f8b" ]; then
        echo -e "${RED}Error: Downloaded file is not a valid gzip archive${NC}"
        echo "Expected gzip magic bytes (1f 8b), got: $FIRST_BYTES"
        echo ""
        
        # Check if it's an HTML error page
        if head -c 100 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null | grep -q "<!DOCTYPE html\|<html\|404\|Not Found"; then
            echo "The downloaded file appears to be an HTML error page."
            echo "This usually means the release doesn't exist yet or the URL is incorrect."
            echo ""
            echo "First few lines of downloaded content:"
            head -n 10 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || true
            echo ""
            echo "Please check:"
            echo "  1. That a release with tag 'latest' exists at: https://github.com/only-using-ai/rustxl/releases"
            echo "  2. That the release includes the file: $ARCHIVE_NAME"
        else
            echo "File content (first 200 bytes):"
            head -c 200 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null | od -c | head -n 5 || true
        fi
        exit 1
    fi
    
    # Additional verification with 'file' command if available
    if command_exists file; then
        FILE_TYPE=$(file -b "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || echo "unknown")
        if [[ "$FILE_TYPE" =~ (HTML|text) ]] && [[ ! "$FILE_TYPE" =~ (gzip|tar|archive|compressed) ]]; then
            echo -e "${RED}Error: Downloaded file appears to be text/HTML, not an archive${NC}"
            echo "File type: $FILE_TYPE"
            echo "First few lines:"
            head -n 5 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || true
            exit 1
        fi
    fi
    
    # Check file size (should be > 0 and reasonable)
    if [ "$(uname)" = "Darwin" ]; then
        FILE_SIZE=$(stat -f%z "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || echo "0")
    else
        FILE_SIZE=$(stat -c%s "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || echo "0")
    fi
    
    if [ "$FILE_SIZE" -lt 1000 ]; then
        echo -e "${RED}Error: Downloaded file is too small ($FILE_SIZE bytes), likely an error page${NC}"
        echo "File content:"
        head -c 500 "$TEMP_DIR/$ARCHIVE_NAME" 2>/dev/null || true
        echo ""
        exit 1
    fi
    
    # Format file size for display
    if command_exists numfmt; then
        FILE_SIZE_DISPLAY=$(numfmt --to=iec-i --suffix=B $FILE_SIZE 2>/dev/null || echo "${FILE_SIZE} bytes")
    else
        FILE_SIZE_DISPLAY="${FILE_SIZE} bytes"
    fi
    
    echo -e "${GREEN}Download complete! ($FILE_SIZE_DISPLAY)${NC}"
    echo ""
    
    # Extract the archive
    echo "Extracting archive..."
    cd "$TEMP_DIR"
    
    # Try to extract and check for errors
    if ! tar -xzf "$ARCHIVE_NAME" 2>&1; then
        echo -e "${RED}Error: Failed to extract archive${NC}"
        echo "This might indicate:"
        echo "  - The downloaded file is corrupted"
        echo "  - The file is not a valid tar.gz archive"
        echo "  - The download was incomplete"
        echo ""
        echo "File type: $(file -b "$ARCHIVE_NAME" 2>/dev/null || echo "unknown")"
        echo "File size: $FILE_SIZE bytes"
        exit 1
    fi
    
    # Find the xl binary - it might be directly in the archive or in a subdirectory
    BINARY_PATH=""

    # Check if xl binary is directly in the current directory
    if [ -f "./xl" ]; then
        BINARY_PATH="./xl"
    else
        # Look for it in a subdirectory
        EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "xl-*" | head -n 1)
        if [ -n "$EXTRACTED_DIR" ] && [ -f "$EXTRACTED_DIR/xl" ]; then
            BINARY_PATH="$EXTRACTED_DIR/xl"
        fi
    fi

    if [ -z "$BINARY_PATH" ]; then
        echo -e "${RED}Error: Could not find xl binary in archive${NC}"
        echo "Archive contents:"
        ls -la
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
        sudo cp "$BINARY_PATH" "$INSTALL_DIR/xl"
        sudo chmod +x "$INSTALL_DIR/xl"
    else
        cp "$BINARY_PATH" "$INSTALL_DIR/xl"
        chmod +x "$INSTALL_DIR/xl"
    fi

    # On macOS, remove quarantine attribute to allow the binary to run
    if [ "$OS" = "macos" ]; then
        if [ "$USE_SUDO" = true ]; then
            sudo xattr -d com.apple.quarantine "$INSTALL_DIR/xl" 2>/dev/null || true
        else
            xattr -d com.apple.quarantine "$INSTALL_DIR/xl" 2>/dev/null || true
        fi
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
