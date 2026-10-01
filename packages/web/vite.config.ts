import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// TG-003 repoints build.rs at `packages/web/dist`, so `outDir` is part of that contract.
//
// The dev proxy forwards the API and the WebSocket to a locally running `cargo run --bin server`
// on its default port 3000. Production never sees this: the built client is embedded and served
// same-origin by the Rust server itself.
//
// TG-601: the service worker is a second entry built to `/sw.js` (a fixed, unhashed name at the
// root — its scope is the whole app). It imports only `src/sw/*`, so it shares no chunk with
// the app and stays a self-contained classic-compatible script.
const DEV_SERVER = 'http://127.0.0.1:3000'

export default defineConfig({
  plugins: [react()],
  server: {
    port: 5174,
    proxy: {
      '/api': DEV_SERVER,
      '/ws': { target: DEV_SERVER, ws: true },
    },
  },
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      input: { main: 'index.html', sw: 'src/sw/sw.ts' },
      output: {
        entryFileNames: (chunk) => (chunk.name === 'sw' ? 'sw.js' : 'assets/[name]-[hash].js'),
      },
    },
  },
})
