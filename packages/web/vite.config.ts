import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// TG-003 repoints build.rs at `packages/web/dist`, so `outDir` is part of that contract.
// Port 5174 keeps this dev server clear of the old Vue client on 5173, which architecture.md
// section 6 wants runnable side by side through M5 as a behaviour reference.
//
// The dev proxy forwards the API, the WebSocket and the embedded static assets (the icon
// sprite lives in web/public and is staged by build.rs, not by this vite root) to a locally
// running `cargo run --bin server` on its default port 3000. Production never sees this:
// the built client is embedded and served same-origin by the Rust server itself.
const DEV_SERVER = 'http://127.0.0.1:3000'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5174,
    proxy: {
      '/api': DEV_SERVER,
      '/ws': { target: DEV_SERVER, ws: true },
      '/icons': DEV_SERVER,
      '/brand': DEV_SERVER,
      '/emoji-data-zh.json': DEV_SERVER,
    },
  },
  build: { outDir: 'dist', emptyOutDir: true },
})
