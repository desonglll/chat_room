import '@tg/ui/styles.css'
import './bench.css'
import { readParams } from './params'
import { sampleFrames } from './metrics'

declare global {
  interface Window {
    __stickerBench?: { ready: boolean; params: unknown; mounted: () => unknown; sample: typeof sampleFrames }
  }
}

const params = readParams(location.search)
const root = document.getElementById('bench')
if (!root) throw new Error('missing #bench')
root.className = 'bench-grid'
root.dataset.offscreen = params.offscreen ? '1' : '0'

const mounts = {
  raw: () => import('./rawBench').then((m) => m.mountRaw),
  managed: () => import('./managedBench').then((m) => m.mountManaged),
  cost: () => import('./costBench').then((m) => m.mountCost),
}
const mount = mounts[params.mode]()

void mount
  .then((run) => run(root, params))
  .then((mounted) => {
    window.__stickerBench = { ready: true, params, mounted, sample: sampleFrames }
  })
