import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  build: {
    outDir: "../../assets",
    emptyOutDir: false,
    cssCodeSplit: false,
    minify: true,
    rollupOptions: {
      input: "src/main.tsx",
      output: {
        entryFileNames: "report-app.js",
        chunkFileNames: "report-app.js",
        assetFileNames: (assetInfo) =>
          assetInfo.name?.endsWith(".css") ? "report-app.css" : "report-app.[ext]"
      }
    }
  }
});
