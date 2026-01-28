import React from "react";
import { AbsoluteFill, useCurrentFrame, useVideoConfig, spring, interpolate, Img, staticFile } from "remotion";
import { TerminalContent } from "./TerminalContent";

// Terminal output completes around frame 60 (2 seconds at 30fps)
const OUTPUT_COMPLETE_FRAME = 60;
const EXIT_START_FRAME = OUTPUT_COMPLETE_FRAME + 3; // Wait 0.1 seconds after output
const TEXT_FADE_START = EXIT_START_FRAME + 8;
const SUBTITLE_FADE_START = TEXT_FADE_START + 12;
const SUBTITLE_COMPLETE = SUBTITLE_FADE_START + 20;
const TEXT_FADE_OUT_START = SUBTITLE_COMPLETE + 24; // 800ms after subtitle appears
const COMMAND_TYPE_START = TEXT_FADE_OUT_START + 15;

const STANDALONE_COMMAND = "ls -Al | xl";
const COMMAND_CHARS_PER_SECOND = 40;

// Image display duration before exit
const IMAGE_DISPLAY_DURATION = 60; // 2 seconds

// Feature text display duration before exit
const FEATURE_TEXT_DISPLAY_DURATION = 60; // 2 seconds

export const MacOSTerminal: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Terminal exit animation - fast spring, no bounce
  const exitProgress = spring({
    frame: Math.max(0, frame - EXIT_START_FRAME),
    fps,
    config: {
      damping: 30,
      stiffness: 200,
      mass: 0.5,
    },
  });

  const terminalY = exitProgress * 1200;

  // Text fade in animations
  const titleOpacity = interpolate(
    frame,
    [TEXT_FADE_START, TEXT_FADE_START + 20],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  const subtitleOpacity = interpolate(
    frame,
    [SUBTITLE_FADE_START, SUBTITLE_FADE_START + 20],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  // Text fade out
  const introTextFadeOut = interpolate(
    frame,
    [TEXT_FADE_OUT_START, TEXT_FADE_OUT_START + 10],
    [1, 0],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  // Standalone command typing
  const commandFramesSinceStart = Math.max(0, frame - COMMAND_TYPE_START);
  const commandCharsToShow = Math.floor((commandFramesSinceStart / fps) * COMMAND_CHARS_PER_SECOND);
  const displayedCommand = STANDALONE_COMMAND.slice(0, Math.min(commandCharsToShow, STANDALONE_COMMAND.length));
  const commandTypingComplete = commandCharsToShow >= STANDALONE_COMMAND.length;

  // Command cursor blink
  const commandCursorVisible = commandTypingComplete
    ? Math.floor(frame / (fps / 2)) % 2 === 0
    : true;

  // Command section opacity (fade in)
  const commandOpacity = interpolate(
    frame,
    [COMMAND_TYPE_START, COMMAND_TYPE_START + 5],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  // Image fade in - 200ms (6 frames) after command typing completes
  const commandTypingDurationFrames = Math.ceil((STANDALONE_COMMAND.length / COMMAND_CHARS_PER_SECOND) * fps);
  const imageStartFrame = COMMAND_TYPE_START + commandTypingDurationFrames + 6;
  const imageOpacity = interpolate(
    frame,
    [imageStartFrame, imageStartFrame + 5],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  // Command fade out as image fades in
  const commandFadeOut = interpolate(
    frame,
    [imageStartFrame, imageStartFrame + 5],
    [1, 0],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  // Image exit animation - push to left
  const imageExitStart = imageStartFrame + IMAGE_DISPLAY_DURATION;
  const imageExitProgress = spring({
    frame: Math.max(0, frame - imageExitStart),
    fps,
    config: {
      damping: 30,
      stiffness: 200,
      mass: 0.5,
    },
  });
  const imageX = imageExitProgress * -1400;

  // Feature text fade in
  const featureTextStart = imageExitStart + 10;
  const feature1Opacity = interpolate(
    frame,
    [featureTextStart, featureTextStart + 15],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );
  const feature2Opacity = interpolate(
    frame,
    [featureTextStart + 12, featureTextStart + 27],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );
  const feature3Opacity = interpolate(
    frame,
    [featureTextStart + 24, featureTextStart + 39],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  // Feature text fade out
  const featureTextFadeOutStart = featureTextStart + 39 + FEATURE_TEXT_DISPLAY_DURATION;
  const featureTextFadeOut = interpolate(
    frame,
    [featureTextFadeOutStart, featureTextFadeOutStart + 10],
    [1, 0],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  // Logo and URL fade in
  const logoStartFrame = featureTextFadeOutStart + 15;
  const logoOpacity = interpolate(
    frame,
    [logoStartFrame, logoStartFrame + 15],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );
  const urlOpacity = interpolate(
    frame,
    [logoStartFrame + 12, logoStartFrame + 27],
    [0, 1],
    { extrapolateLeft: "clamp", extrapolateRight: "clamp" }
  );

  return (
    <AbsoluteFill
      style={{
        background: `
          radial-gradient(ellipse at 20% 20%, rgba(241, 237, 248, 0.8) 0%, transparent 50%),
          radial-gradient(ellipse at 80% 30%, rgba(233, 229, 245, 0.7) 0%, transparent 45%),
          radial-gradient(ellipse at 40% 80%, rgba(245, 241, 251, 0.8) 0%, transparent 50%),
          radial-gradient(ellipse at 90% 80%, rgba(229, 225, 241, 0.6) 0%, transparent 40%),
          radial-gradient(ellipse at 50% 50%, rgba(239, 235, 247, 0.9) 0%, transparent 60%),
          linear-gradient(135deg, #f5f2f9 0%, #eee9f6 25%, #e9e4f3 50%, #f1eef7 75%, #f9f7fc 100%)
        `,
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        padding: 40,
      }}
    >
      {/* Intro Text Content */}
      <div
        style={{
          position: "absolute",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          zIndex: 0,
          opacity: introTextFadeOut,
        }}
      >
        <div
          style={{
            fontSize: 72,
            fontWeight: 600,
            color: "#2d2d2d",
            fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            opacity: titleOpacity,
          }}
        >
          Introducing XL.
        </div>
        <div
          style={{
            fontSize: 36,
            fontWeight: 400,
            color: "#5a5a5a",
            fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            marginTop: 20,
            opacity: subtitleOpacity,
          }}
        >
          Excel in your terminal
        </div>
      </div>

      {/* Standalone Command */}
      <div
        style={{
          position: "absolute",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          zIndex: 0,
          opacity: commandOpacity * commandFadeOut,
          fontFamily: '"SF Mono", "Monaco", "Menlo", "Courier New", monospace',
          fontSize: 64,
          color: "#2d2d2d",
        }}
      >
        <span>{displayedCommand}</span>
        <span
          style={{
            width: 32,
            height: 72,
            backgroundColor: commandCursorVisible ? "#2d2d2d" : "transparent",
            display: "inline-block",
            marginLeft: 4,
          }}
        />
      </div>

      {/* ls-pipe image */}
      <Img
        src={staticFile("ls-pipe.png")}
        style={{
          position: "absolute",
          width: "calc(100% - 100px)",
          height: "calc(100% - 100px)",
          objectFit: "contain",
          opacity: imageOpacity,
          transform: `translateX(${imageX}px)`,
          zIndex: 2,
        }}
      />

      {/* Feature Text */}
      <div
        style={{
          position: "absolute",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          zIndex: 3,
          opacity: featureTextFadeOut,
        }}
      >
        <div
          style={{
            fontSize: 56,
            fontWeight: 600,
            color: "#2d2d2d",
            fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            opacity: feature1Opacity,
            marginBottom: 24,
          }}
        >
          Formula support.
        </div>
        <div
          style={{
            fontSize: 56,
            fontWeight: 600,
            color: "#2d2d2d",
            fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            opacity: feature2Opacity,
            marginBottom: 24,
          }}
        >
          VIM-like syntax.
        </div>
        <div
          style={{
            fontSize: 56,
            fontWeight: 600,
            color: "#2d2d2d",
            fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            opacity: feature3Opacity,
          }}
        >
          Written in Rust.
        </div>
      </div>

      {/* Logo and URL */}
      <div
        style={{
          position: "absolute",
          display: "flex",
          flexDirection: "column",
          alignItems: "center",
          justifyContent: "center",
          zIndex: 4,
        }}
      >
        <Img
          src={staticFile("logo.svg")}
          style={{
            width: 300,
            height: 300,
            objectFit: "contain",
            opacity: logoOpacity,
          }}
        />
        <div
          style={{
            fontSize: 36,
            fontWeight: 500,
            color: "#2d2d2d",
            fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            marginTop: 30,
            opacity: urlOpacity,
          }}
        >
          https://rustxl.com/
        </div>
      </div>

      {/* Terminal Window */}
      <div
        style={{
          width: "100%",
          height: "100%",
          backgroundColor: "#ffffff",
          borderRadius: 10,
          boxShadow: "0 40px 80px rgba(0, 0, 0, 0.4)",
          display: "flex",
          flexDirection: "column",
          overflow: "hidden",
          transform: `translateY(${terminalY}px)`,
          zIndex: 1,
        }}
      >
        {/* Title Bar */}
        <div
          style={{
            height: 52,
            backgroundColor: "#f6f6f6",
            borderBottom: "1px solid #d4d4d4",
            display: "flex",
            alignItems: "center",
            padding: "0 16px",
            position: "relative",
          }}
        >
          {/* Traffic Lights */}
          <div style={{ display: "flex", gap: 8 }}>
            <div
              style={{
                width: 14,
                height: 14,
                borderRadius: "50%",
                backgroundColor: "#ff5f56",
                border: "1px solid #e14942",
              }}
            />
            <div
              style={{
                width: 14,
                height: 14,
                borderRadius: "50%",
                backgroundColor: "#ffbd2e",
                border: "1px solid #dfa123",
              }}
            />
            <div
              style={{
                width: 14,
                height: 14,
                borderRadius: "50%",
                backgroundColor: "#27c93f",
                border: "1px solid #1dad2b",
              }}
            />
          </div>

          {/* Window Title */}
          <div
            style={{
              position: "absolute",
              left: "50%",
              transform: "translateX(-50%)",
              fontSize: 14,
              fontWeight: 500,
              color: "#4d4d4d",
              fontFamily:
                '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif',
            }}
          >
            Terminal
          </div>
        </div>

        {/* Terminal Content Area */}
        <TerminalContent />
      </div>
    </AbsoluteFill>
  );
};
