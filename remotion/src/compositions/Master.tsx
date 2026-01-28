import React from "react";
import { AbsoluteFill, Series, spring, useCurrentFrame, useVideoConfig } from "remotion";
import { MacOSTerminal } from "./MacOSTerminal";
import { Logo } from "./Logo";

export const Master: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = useVideoConfig();

  // Fast spring with no bounce (overdamped)
  const slideIn = spring({
    frame,
    fps,
    config: {
      damping: 30,
      stiffness: 200,
      mass: 0.5,
    },
  });

  const translateY = (1 - slideIn) * 1200;

  return (
    <AbsoluteFill
      style={{
        backgroundColor: "#3a3a3a",
        perspective: 2000,
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
      }}
    >
      <div
        style={{
          width: "100%",
          height: "100%",
          transform: `rotateX(20deg) rotateY(-20deg) translateY(${translateY}px)`,
          transformStyle: "preserve-3d",
        }}
      >
        <Series>
          <Series.Sequence durationInFrames={300}>
            <MacOSTerminal />
          </Series.Sequence>
          <Series.Sequence durationInFrames={150}>
            <Logo />
          </Series.Sequence>
        </Series>
      </div>
    </AbsoluteFill>
  );
};
