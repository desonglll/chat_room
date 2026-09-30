/**
 * Fixture media for the mixed-format page, generated in the browser at load time — no binary
 * or copyrighted sticker is checked in. WebP comes from `canvas.toBlob('image/webp')`, WebM
 * from `MediaRecorder` recording an animated, transparent canvas (VP9, as Telegram's WebM
 * stickers are).
 */

const SIZE = 256

/** Hue (degrees) → RGB bytes at fixed saturation/lightness. */
function hueRgb(hue: number): [number, number, number] {
  const f = (n: number) => {
    const k = (n + hue / 30) % 12
    return Math.round(255 * (0.55 - 0.45 * Math.max(-1, Math.min(k - 3, 9 - k, 1))))
  }
  return [f(0), f(8), f(4)]
}

/**
 * A bouncing smiley on a transparent ground, written as pixel data (colours here are data,
 * not CSS — the token-discipline gate forbids colour literals in this package).
 */
function drawFace(context: CanvasRenderingContext2D, hue: number, phase: number) {
  const image = context.createImageData(SIZE, SIZE)
  const [r, g, b] = hueRgb(hue)
  const cx = SIZE / 2
  const cy = SIZE / 2 + Math.sin(phase * Math.PI * 2) * 18
  const radius = SIZE * 0.36
  for (let y = 0; y < SIZE; y += 1) {
    for (let x = 0; x < SIZE; x += 1) {
      const dx = x - cx
      const dy = y - cy
      if (dx * dx + dy * dy > radius * radius) continue
      const eye = Math.hypot(Math.abs(dx) - 34, dy + 18) < 10
      const mouthR = Math.hypot(dx, dy - 12)
      const mouth = mouthR > 36 && mouthR < 44 && dy - 12 > 18
      const ink = eye || mouth
      const offset = (y * SIZE + x) * 4
      image.data[offset] = ink ? 34 : r
      image.data[offset + 1] = ink ? 34 : g
      image.data[offset + 2] = ink ? 34 : b
      image.data[offset + 3] = 255
    }
  }
  context.putImageData(image, 0, 0)
}

function canvas2d(): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const canvas = document.createElement('canvas')
  canvas.width = SIZE
  canvas.height = SIZE
  const context = canvas.getContext('2d')
  if (!context) throw new Error('no 2d context')
  return [canvas, context]
}

const bytesOf = async (blob: Blob) => new Uint8Array(await blob.arrayBuffer())

export async function makeWebp(hue: number): Promise<Uint8Array> {
  const [canvas, context] = canvas2d()
  drawFace(context, hue, 0.25)
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/webp', 0.9))
  if (!blob || blob.type !== 'image/webp') throw new Error('canvas cannot encode WebP here')
  return bytesOf(blob)
}

export async function makeWebm(hue: number, durationMs = 1200): Promise<Uint8Array> {
  const [canvas, context] = canvas2d()
  const type = ['video/webm;codecs=vp9', 'video/webm;codecs=vp8', 'video/webm'].find((candidate) =>
    MediaRecorder.isTypeSupported(candidate),
  )
  if (!type) throw new Error('MediaRecorder cannot record WebM here')
  const recorder = new MediaRecorder(canvas.captureStream(30), { mimeType: type })
  const chunks: Blob[] = []
  recorder.ondataavailable = (event) => chunks.push(event.data)
  const stopped = new Promise((resolve) => (recorder.onstop = resolve))
  const start = performance.now()
  let frame = 0
  const draw = () => {
    const elapsed = performance.now() - start
    drawFace(context, hue, elapsed / durationMs)
    if (elapsed < durationMs) frame = requestAnimationFrame(draw)
    else recorder.stop()
  }
  recorder.start()
  draw()
  await stopped
  cancelAnimationFrame(frame)
  return bytesOf(new Blob(chunks, { type: 'video/webm' }))
}
