import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  base: "./",
  plugins: [react()],
  build: {
    assetsInlineLimit: 0,
    cssCodeSplit: false,
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: (assetInfo) =>
          assetInfo.names.some((name) => name.endsWith(".css"))
            ? "assets/app.css"
            : assetInfo.names.some((name) => name.includes("basier-square-regular"))
              ? "assets/basier-square-regular.woff2"
              : assetInfo.names.some((name) => name.includes("basier-square-semibold"))
                ? "assets/basier-square-semibold.woff2"
            : "assets/[name][extname]",
        manualChunks: undefined,
      },
    },
  },
  test: {
    environment: "jsdom",
    setupFiles: "./tests/setup.ts",
    css: true,
  },
});
