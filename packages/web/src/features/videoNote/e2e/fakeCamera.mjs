/**
 * A NON-SQUARE fake camera for Chromium's `--use-file-for-fake-video-capture` (TG-402 E2E):
 * a 640×360 Y4M whose centred 360×360 square is green (with a moving white block) and whose
 * two 140 px side strips are red (left) and blue (right). A correct centre crop shows only
 * green; a squash or an off-centre crop shows red or blue at the circle's edge.
 */
import { writeFileSync } from 'node:fs'

export const CAMERA = { width: 640, height: 360 }

const clampByte = (value) => Math.max(0, Math.min(255, Math.round(value)))
const toYuv = (r, g, b) => [
  clampByte(0.299 * r + 0.587 * g + 0.114 * b),
  clampByte(128 - 0.168736 * r - 0.331264 * g + 0.5 * b),
  clampByte(128 + 0.5 * r - 0.418688 * g - 0.081312 * b),
]

export function writeFakeCamera(path, { width, height } = CAMERA, frames = 60) {
  const side = Math.min(width, height)
  const x0 = (width - side) / 2
  const y0 = (height - side) / 2
  const colour = (x, y, frame) => {
    if (x < x0 || y < y0) return [220, 30, 30]
    if (x >= x0 + side || y >= y0 + side) return [30, 60, 220]
    const bx = x0 + side / 2 - 40 + Math.round(60 * Math.sin(frame / 8))
    const by = y0 + side / 2 - 40
    if (x >= bx && x < bx + 80 && y >= by && y < by + 80) return [250, 250, 250]
    return [30, 190, 60]
  }
  const parts = [Buffer.from(`YUV4MPEG2 W${width} H${height} F30:1 Ip A1:1 C420jpeg\n`)]
  for (let frame = 0; frame < frames; frame += 1) {
    const luma = Buffer.alloc(width * height)
    const u = Buffer.alloc((width * height) / 4)
    const v = Buffer.alloc((width * height) / 4)
    for (let y = 0; y < height; y += 1) {
      for (let x = 0; x < width; x += 1) {
        const [yy, uu, vv] = toYuv(...colour(x, y, frame))
        luma[y * width + x] = yy
        if (x % 2 === 0 && y % 2 === 0) {
          u[(y / 2) * (width / 2) + x / 2] = uu
          v[(y / 2) * (width / 2) + x / 2] = vv
        }
      }
    }
    parts.push(Buffer.from('FRAME\n'), luma, u, v)
  }
  writeFileSync(path, Buffer.concat(parts))
}

/** Which of the three test colours an RGB sample is closest to. */
export function classify([r, g, b]) {
  if (g > r + 60 && g > b + 60) return 'green'
  if (r > g + 60 && r > b + 60) return 'red'
  if (b > r + 60 && b > g + 60) return 'blue'
  if (r > 200 && g > 200 && b > 200) return 'white'
  return `other(${r},${g},${b})`
}

/**
 * Installs `window.__edgeProbe(source, side)` in a page (evaluated as a string: the app's CSP
 * forbids eval, the DevTools protocol does not): the colours 4 px inside the four edge
 * midpoints of a square drawable (canvas, video frame, image).
 */
export const EDGE_PROBE = `window.__edgeProbe = (source, side) => {
  const canvas = document.createElement('canvas')
  canvas.width = side
  canvas.height = side
  const context = canvas.getContext('2d')
  context.drawImage(source, 0, 0, side, side)
  const at = (x, y) => Array.from(context.getImageData(x, y, 1, 1).data.slice(0, 3))
  const m = Math.round(side / 2)
  return { left: at(4, m), right: at(side - 5, m), top: at(m, 4), bottom: at(m, side - 5) }
}`

export function assertAllGreen(edges, where) {
  const named = Object.fromEntries(Object.entries(edges).map(([edge, rgb]) => [edge, classify(rgb)]))
  if (Object.values(named).some((colour) => colour !== 'green')) {
    throw new Error(`${where} is not the centred square: ${JSON.stringify(named)}`)
  }
  return named
}
