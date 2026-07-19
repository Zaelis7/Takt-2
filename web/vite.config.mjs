import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  build: {
    assetsDir: "assets",
    emptyOutDir: true,
    sourcemap: false,
  },
  plugins: [react()],
});
