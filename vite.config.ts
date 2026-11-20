import { defineConfig } from "vite";

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: "127.0.0.1",
    watch: {
      // Cargo creates and locks .exe files during builds on Windows.
      // Never watch Rust build artifacts.
      ignored: ["**/src-tauri/target/**", "**/target/**"],
    },
  },
  envPrefix: ["VITE_", "TAURI_ENV_*"],
});
