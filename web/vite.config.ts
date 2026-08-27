import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import tailwindcss from '@tailwindcss/vite'
import { readFileSync } from 'node:fs'
import { fileURLToPath } from 'node:url'
import { buildServiceWorker, STATIC_PWA_ASSET_URLS, type PrecacheAsset } from './src/pwaBuild.ts'

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag === 'emoji-picker',
        },
      },
    }),
    tailwindcss(),
    {
      name: 'echo-gate-service-worker',
      apply: 'build',
      generateBundle(_options, bundle) {
        const assets: PrecacheAsset[] = Object.values(bundle)
          .filter((output) => output.fileName.startsWith('assets/'))
          .map((output) => ({
            url: `/${output.fileName}`,
            content: output.type === 'chunk' ? output.code : output.source,
          }))
        for (const url of STATIC_PWA_ASSET_URLS) {
          assets.push({
            url,
            content: readFileSync(fileURLToPath(new URL(`./public${url}`, import.meta.url))),
          })
        }
        this.emitFile({ type: 'asset', fileName: 'sw.js', source: buildServiceWorker(assets) })
      },
    },
  ],
  server: {
    proxy: {
      '/api': 'http://127.0.0.1:3000',
      '/ws': {
        target: 'ws://127.0.0.1:3000',
        ws: true,
      },
    },
  },
  build: {
    cssCodeSplit: false,
    rollupOptions: {
      output: {
        entryFileNames: 'assets/app.js',
        chunkFileNames: 'assets/[name].js',
        assetFileNames: (assetInfo) =>
          assetInfo.names.some((name) => name.endsWith('.css')) ? 'assets/app.css' : 'assets/[name][extname]',
      },
    },
  },
})
