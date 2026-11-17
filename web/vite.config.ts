import { defineConfig, loadEnv } from "vite";
import react from "@vitejs/plugin-react";

// Locally the dev server proxies /api to the Rust server so the browser only
// talks to one origin. Set VITE_API_PROXY to point at another backend (e.g. the
// Pi's cloudflare tunnel hostname) when developing against it.
export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, process.cwd(), "");
  const target = env.VITE_API_PROXY || "http://127.0.0.1:8080";
  return {
    plugins: [react()],
    server: {
      port: 5173,
      strictPort: true,
      proxy: {
        "/api": { target, changeOrigin: true },
      },
    },
    preview: {
      port: 5173,
      proxy: {
        "/api": { target, changeOrigin: true },
      },
    },
  };
});
