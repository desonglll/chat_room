import react from '@vitejs/plugin-react'
import { defineConfig } from 'vite'

// TG-003 repoints build.rs at `packages/web/dist`, so `outDir` is part of that contract.
// Port 5174 keeps this dev server clear of the old Vue client on 5173, which architecture.md
// section 6 wants runnable side by side through M5 as a behaviour reference.
export default defineConfig({
  plugins: [react()],
  server: { port: 5174 },
  build: { outDir: 'dist', emptyOutDir: true },
})
