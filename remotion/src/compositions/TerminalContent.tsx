import React from "react";
import { useCurrentFrame, useVideoConfig, interpolate } from "remotion";

const COMMAND = "curl -fsSL http://rustxl.com/install.sh | bash";
const CHARS_PER_SECOND = 54; // 43 chars in 800ms
const OUTPUT_DELAY_MS = 200; // Delay after typing before output appears
const MS_PER_LINE = 24; // Time between each line appearing (21 lines in ~500ms)

const OUTPUT_LINES = [
  "",
  "rustxl Installation Script",
  "================================",
  "",
  "Detected OS: macos",
  "Detected Architecture: aarch64",
  "",
  "Download URL: https://github.com/only-using-ai/rustxl/releases/download/latest/xl-macos-arm64.tar.gz",
  "",
  "Downloading rustxl...",
  "Download complete! (1076107 bytes)",
  "",
  "Extracting archive...",
  "Extraction complete!",
  "",
  "Installing xl to /Users/willdech/.local/bin...",
  "",
  "Installation successful!",
  "",
  "The 'xl' command is now available.",
  "You can verify by running: xl --help",
];

export const TerminalContent: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Calculate how many characters to show based on current frame
  const charsToShow = Math.floor((frame / fps) * CHARS_PER_SECOND);
  const displayedText = COMMAND.slice(0, Math.min(charsToShow, COMMAND.length));
  const isTypingComplete = charsToShow >= COMMAND.length;

  // Calculate typing completion time in ms
  const typingDurationMs = (COMMAND.length / CHARS_PER_SECOND) * 1000;
  const currentTimeMs = (frame / fps) * 1000;

  // Calculate how many output lines to show
  const timeSinceTypingComplete = currentTimeMs - typingDurationMs - OUTPUT_DELAY_MS;
  const linesToShow = timeSinceTypingComplete > 0
    ? Math.min(Math.floor(timeSinceTypingComplete / MS_PER_LINE) + 1, OUTPUT_LINES.length)
    : 0;

  const visibleOutput = OUTPUT_LINES.slice(0, linesToShow);
  const outputComplete = linesToShow >= OUTPUT_LINES.length;

  // Animate font size reduction during output generation (32px -> 22.4px, 70%)
  const fontSize = interpolate(
    linesToShow,
    [0, OUTPUT_LINES.length],
    [32, 22.4],
    { extrapolateRight: "clamp" }
  );
  const cursorHeight = fontSize * 1.2;
  const cursorWidth = fontSize * 0.5;
  const lineHeight = fontSize * 1.5;

  // Cursor blink effect (only after all output is complete)
  const cursorVisible = outputComplete
    ? Math.floor(frame / (fps / 2)) % 2 === 0
    : true;

  // Hide cursor on command line once output starts
  const showCommandCursor = !isTypingComplete || (isTypingComplete && linesToShow === 0);

  return (
    <div
      style={{
        flex: 1,
        backgroundColor: "#ffffff",
        padding: 20,
        fontFamily: '"SF Mono", "Monaco", "Menlo", "Courier New", monospace',
        fontSize,
        lineHeight: 1.5,
        color: "#000000",
        overflow: "hidden",
      }}
    >
      {/* Prompt Line */}
      <div style={{ display: "flex", alignItems: "center" }}>
        <span style={{ color: "#000000" }}>~ $</span>
        <span style={{ marginLeft: fontSize * 0.5 }}>{displayedText}</span>
        {showCommandCursor && (
          <span
            style={{
              width: cursorWidth,
              height: cursorHeight,
              backgroundColor: "#000000",
              display: "inline-block",
              marginLeft: 2,
            }}
          />
        )}
      </div>

      {/* Output Lines */}
      {visibleOutput.map((line, index) => (
        <div key={index} style={{ minHeight: lineHeight }}>
          {line}
        </div>
      ))}

      {/* Cursor after output */}
      {linesToShow > 0 && (
        <div style={{ display: "flex", alignItems: "center", marginTop: 8 }}>
          <span style={{ color: "#000000" }}>~ $</span>
          <span
            style={{
              width: cursorWidth,
              height: cursorHeight,
              backgroundColor: cursorVisible ? "#000000" : "transparent",
              display: "inline-block",
              marginLeft: fontSize * 0.5,
            }}
          />
        </div>
      )}
    </div>
  );
};
