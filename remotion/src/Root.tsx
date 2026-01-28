import React from "react";
import { Composition } from "remotion";
import { MacOSTerminal } from "./compositions/MacOSTerminal";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="MacOSTerminal"
        component={MacOSTerminal}
        durationInFrames={450}
        fps={30}
        width={1280}
        height={1000}
      />
    </>
  );
};
