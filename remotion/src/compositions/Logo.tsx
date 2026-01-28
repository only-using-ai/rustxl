import React from "react";
import { AbsoluteFill, Img, staticFile } from "remotion";

export const Logo: React.FC = () => {
  return (
    <AbsoluteFill
      style={{
        backgroundColor: "transparent",
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
      }}
    >
      <Img
        src={staticFile("logo.svg")}
        style={{
          maxWidth: "80%",
          maxHeight: "80%",
          objectFit: "contain",
        }}
      />
    </AbsoluteFill>
  );
};
