import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  server: {
    host: "0.0.0.0",
    port: 5178,
    proxy: {
      "/api": "http://127.0.0.1:8788",
      "/health": "http://127.0.0.1:8788",
      "/event": "http://127.0.0.1:8788"
    }
  },
  test: {
    environment: "jsdom"
  }
});
