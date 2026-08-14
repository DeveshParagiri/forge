import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vite-plus/test";

import config from "../../app.config";

type PngMetadata = {
  readonly alpha: boolean;
  readonly height: number;
  readonly width: number;
};

const mobileRoot = new URL("../../", import.meta.url);

function asset(relativePath: string): Buffer {
  return readFileSync(new URL(relativePath, mobileRoot));
}

function pngMetadata(relativePath: string): PngMetadata {
  const bytes = asset(relativePath);
  expect(bytes.subarray(1, 4).toString("ascii")).toBe("PNG");
  const colorType = bytes.readUInt8(25);
  return {
    alpha: colorType === 4 || colorType === 6,
    height: bytes.readUInt32BE(20),
    width: bytes.readUInt32BE(16),
  };
}

function sha256(relativePath: string): string {
  return createHash("sha256").update(asset(relativePath)).digest("hex");
}

describe("Forge mobile brand assets", () => {
  it("keeps the supplied source and platform derivatives at their required geometry", () => {
    expect(pngMetadata("../../brand/mobile-logo-source.png")).toEqual({
      alpha: false,
      height: 2000,
      width: 2000,
    });
    expect(sha256("../../brand/mobile-logo-source.png")).toBe(
      "4d9023783cef8d6d0dd472ebeb4c253094698e06715345b9b8aa34b64a9cae9a",
    );
    expect(pngMetadata("assets/forge/icon.png")).toEqual({
      alpha: false,
      height: 1024,
      width: 1024,
    });
    expect(pngMetadata("assets/forge/mark.png")).toEqual({
      alpha: true,
      height: 1024,
      width: 1024,
    });
    expect(pngMetadata("assets/forge/adaptive-icon-foreground.png")).toEqual({
      alpha: true,
      height: 432,
      width: 432,
    });
    expect(sha256("assets/forge/adaptive-icon-monochrome.png")).toBe(
      sha256("assets/forge/adaptive-icon-foreground.png"),
    );
  });

  it("references the full icon and transparent marks from Expo config", () => {
    expect(config.icon).toBe("./assets/forge/icon.png");
    expect(config.android?.adaptiveIcon).toEqual({
      backgroundColor: "#000007",
      foregroundImage: "./assets/forge/adaptive-icon-foreground.png",
      monochromeImage: "./assets/forge/adaptive-icon-monochrome.png",
    });

    const splash = config.plugins?.find(
      (plugin): plugin is [string, Record<string, unknown>] =>
        Array.isArray(plugin) && plugin[0] === "expo-splash-screen",
    );
    expect(splash?.[1]).toMatchObject({
      backgroundColor: "#000000",
      image: "./assets/forge/mark.png",
      imageWidth: 180,
      dark: {
        backgroundColor: "#000000",
        image: "./assets/forge/mark.png",
      },
    });
  });
});
