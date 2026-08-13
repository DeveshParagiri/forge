const fs = require("node:fs");
const path = require("node:path");

const { withDangerousMod } = require("expo/config-plugins");

const MARKER = "# forge: link Expo Camera's iOS barcode-scanning companion";
const POD_DECLARATION = `  ${MARKER}
  expo_camera_ios_path = File.join(
    File.dirname(\`node --print "require.resolve('expo-camera/package.json')"\`),
    "ios",
  )
  pod 'ZXingObjC/PDF417', :modular_headers => true
  pod 'ZXingObjC/OneD', :modular_headers => true
  pod 'ExpoCameraBarcodeScanning', :path => expo_camera_ios_path
`;

module.exports = function withIosCameraBarcodeScanning(config) {
  return withDangerousMod(config, [
    "ios",
    (nextConfig) => {
      const podfilePath = path.join(nextConfig.modRequest.platformProjectRoot, "Podfile");
      const podfile = fs.readFileSync(podfilePath, "utf8");

      if (podfile.includes(MARKER)) {
        return nextConfig;
      }

      const targetStart = `target '${nextConfig.name}' do\n`;
      if (!podfile.includes(targetStart)) {
        throw new Error(
          `Unable to link iOS barcode scanning: ${nextConfig.name} target is missing from Podfile.`,
        );
      }

      fs.writeFileSync(
        podfilePath,
        podfile.replace(targetStart, `${targetStart}${POD_DECLARATION}`),
        "utf8",
      );
      return nextConfig;
    },
  ]);
};
