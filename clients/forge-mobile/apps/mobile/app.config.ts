import type { ExpoConfig } from "expo/config";

const config: ExpoConfig = {
  name: "Forge",
  slug: "forge-mobile",
  version: "0.1.0",
  orientation: "portrait",
  platforms: ["ios", "android"],
  scheme: "forge",
  icon: "./assets/forge/icon.png",
  userInterfaceStyle: "automatic",
  ios: {
    bundleIdentifier: "com.exaforge.forge.dev",
    supportsTablet: true,
    infoPlist: {
      ITSAppUsesNonExemptEncryption: false,
      NSLocalNetworkUsageDescription:
        "Forge connects to private remote sessions on your Tailscale network.",
    },
  },
  android: {
    package: "com.exaforge.forge.dev",
    adaptiveIcon: {
      foregroundImage: "./assets/forge/icon.png",
      backgroundColor: "#000000",
    },
    predictiveBackGestureEnabled: true,
  },
  plugins: [
    "expo-asset",
    [
      "expo-font",
      {
        ios: {
          fonts: [
            "./assets/forge/BasierSquare-Regular.otf",
            "./assets/forge/BasierSquare-Semibold.otf",
            "./assets/forge/BasierSquareMono-Regular.ttf",
          ],
        },
        android: {
          fonts: [
            {
              fontFamily: "Basier Square",
              fontDefinitions: [
                { path: "./assets/forge/BasierSquare-Regular.otf", weight: 400 },
                { path: "./assets/forge/BasierSquare-Semibold.otf", weight: 600 },
              ],
            },
            {
              fontFamily: "Basier Square Mono",
              fontDefinitions: [
                { path: "./assets/forge/BasierSquareMono-Regular.ttf", weight: 400 },
              ],
            },
          ],
        },
      },
    ],
    "expo-secure-store",
    [
      "expo-camera",
      {
        cameraPermission: "Allow Forge to scan a private session pairing code.",
        microphonePermission: false,
        barcodeScannerEnabled: true,
        recordAudioAndroid: false,
      },
    ],
    "./plugins/withIosCameraBarcodeScanning.cjs",
    [
      "expo-splash-screen",
      {
        image: "./assets/forge/icon.png",
        resizeMode: "contain",
        backgroundColor: "#000000",
        imageWidth: 180,
        dark: {
          image: "./assets/forge/icon.png",
          backgroundColor: "#000000",
        },
      },
    ],
    [
      "expo-build-properties",
      {
        ios: { deploymentTarget: "18.0" },
      },
    ],
    "./plugins/withIosCocoaPodsUuidCache.cjs",
    "./plugins/withIosSceneLifecycle.cjs",
  ],
};

export default config;
