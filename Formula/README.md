# Homebrew Formula for xl

This directory contains the Homebrew formula for installing `xl` via `brew install xl`.

## Setup Options

### Option 1: Create a Homebrew Tap (Recommended for personal use)

1. Create a new GitHub repository named `homebrew-xl`:
   ```bash
   # Create the repository on GitHub first, then:
   mkdir ~/homebrew-xl
   cd ~/homebrew-xl
   git init
   git remote add origin https://github.com/YOUR_USERNAME/homebrew-xl.git
   ```

2. Copy the formula to the tap:
   ```bash
   mkdir -p ~/homebrew-xl/Formula
   cp Formula/xl.rb ~/homebrew-xl/Formula/
   ```

3. Commit and push:
   ```bash
   cd ~/homebrew-xl
   git add Formula/xl.rb
   git commit -m "Add xl formula"
   git push -u origin main
   ```

4. Users can then install with:
   ```bash
   brew tap YOUR_USERNAME/xl
   brew install xl
   ```

### Option 2: Submit to homebrew-core (Official Homebrew)

To submit to the official Homebrew repository:

1. Fork the [homebrew-core](https://github.com/Homebrew/homebrew-core) repository
2. Copy `Formula/xl.rb` to your fork
3. Update the SHA256 values by running:
   ```bash
   brew fetch --force YOUR_USERNAME/homebrew-core/xl
   ```
4. Follow the [Homebrew contribution guidelines](https://docs.brew.sh/Adding-Software-to-Homebrew)
5. Submit a pull request

## Updating the Formula

When you create a new release:

1. Update the `version` in `Formula/xl.rb`
2. Update the URLs to point to the new release
3. Calculate the SHA256 checksums:
   ```bash
   # For Intel Mac
   curl -L https://github.com/only-using-ai/rustxl/releases/download/v0.1.0/xl-macos-x86_64.tar.gz | shasum -a 256
   
   # For Apple Silicon Mac
   curl -L https://github.com/only-using-ai/rustxl/releases/download/v0.1.0/xl-macos-arm64.tar.gz | shasum -a 256
   ```
4. Update the `sha256` values in the formula
5. Test the formula:
   ```bash
   brew install --build-from-source Formula/xl.rb
   ```

## Testing Locally

You can test the formula locally before publishing:

```bash
# Install from local formula file
brew install --build-from-source Formula/xl.rb

# Or if using a tap
brew install --build-from-source YOUR_USERNAME/xl/xl
```

## Notes

- The formula currently uses version `0.1.0` - update this when creating releases
- SHA256 checksums need to be filled in after the first release is created
- The formula supports both Intel (x86_64) and Apple Silicon (arm64) Macs
