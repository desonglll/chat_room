/**
 * Deterministic synthetic Lottie documents for tests and the benchmark page.
 *
 * No real Telegram stickers are committed (they are copyrighted). Instead this generator
 * produces animations with the *cost profile* of a typical TGS sticker: 512×512, 60 fps, 3 s,
 * a few dozen shape layers, each a closed bezier path that morphs between keyframes, with an
 * animated transform, a fill, and on some layers a stroke. Complexity is a knob so the
 * benchmark can model both a light emoji-like sticker and a heavy one.
 *
 * Colours are Lottie RGBA float arrays, i.e. data, not CSS.
 */

export interface SyntheticOptions {
  seed: number
  /** Shape layers. Typical TGS stickers carry roughly 15–60. */
  layers?: number
  /** Bezier vertices per path. */
  vertices?: number
  frameRate?: number
  frames?: number
  size?: number
}

type Vec = [number, number]
type Rng = () => number

/** mulberry32: tiny, deterministic, good enough for shapes. */
function rng(seed: number): Rng {
  let state = seed >>> 0
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let t = state
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

const round = (n: number) => Math.round(n * 100) / 100
const ease = { i: { x: [0.4], y: [1] }, o: { x: [0.6], y: [0] } }
const pathEase = { i: { x: 0.4, y: 1 }, o: { x: 0.6, y: 0 } }
const still = (k: unknown) => ({ a: 0, k })

function blob(random: Rng, vertices: number, radius: number) {
  const v: Vec[] = []
  const i: Vec[] = []
  const o: Vec[] = []
  for (let n = 0; n < vertices; n += 1) {
    const angle = (n / vertices) * Math.PI * 2
    const r = radius * (0.65 + random() * 0.5)
    const tangent = ((Math.PI * 2 * r) / vertices) * 0.38
    v.push([round(Math.cos(angle) * r), round(Math.sin(angle) * r)])
    i.push([round(Math.sin(angle) * tangent), round(-Math.cos(angle) * tangent)])
    o.push([round(-Math.sin(angle) * tangent), round(Math.cos(angle) * tangent)])
  }
  return { i, o, v, c: true }
}

function animated<T>(frames: number, values: T[], easing: object) {
  const step = frames / (values.length - 1)
  return {
    a: 1,
    k: values.map((s, index) =>
      index === values.length - 1 ? { t: Math.round(step * index), s } : { t: Math.round(step * index), s, ...easing },
    ),
  }
}

function shapeLayer(random: Rng, index: number, options: Required<Omit<SyntheticOptions, 'seed'>>) {
  const { size, frames, vertices } = options
  const radius = size * (0.06 + random() * 0.14)
  const centre = (): number[] => [round(size * (0.2 + random() * 0.6)), round(size * (0.2 + random() * 0.6)), 0]
  const colour = () => [round(random()), round(random()), round(random()), 1]
  const items: unknown[] = [
    {
      ty: 'sh',
      nm: 'path',
      ks: animated(
        frames,
        [0, 1, 2].map(() => [blob(random, vertices, radius)]),
        pathEase,
      ),
    },
    { ty: 'fl', nm: 'fill', c: animated(frames, [colour(), colour()], ease), o: still(100), r: 1 },
  ]
  if (index % 3 === 0) {
    items.push({
      ty: 'st',
      nm: 'stroke',
      c: still(colour()),
      o: still(100),
      w: still(round(2 + random() * 6)),
      lc: 2,
      lj: 2,
    })
  }
  items.push({
    ty: 'tr',
    p: still([0, 0]),
    a: still([0, 0]),
    s: still([100, 100]),
    r: still(0),
    o: still(100),
    sk: still(0),
    sa: still(0),
  })
  const start = centre()
  const spin = round((random() - 0.5) * 360)
  return {
    ddd: 0,
    ind: index + 1,
    ty: 4,
    nm: `layer ${index + 1}`,
    sr: 1,
    ao: 0,
    ip: 0,
    op: frames,
    st: 0,
    bm: 0,
    ks: {
      o: still(100),
      r: animated(frames, [[0], [spin], [0]], ease),
      p: animated(frames, [start, centre(), start], ease),
      a: still([0, 0, 0]),
      s: animated(
        frames,
        [
          [100, 100, 100],
          [70 + random() * 60, 70 + random() * 60, 100],
          [100, 100, 100],
        ],
        ease,
      ),
    },
    shapes: [{ ty: 'gr', nm: 'group', it: items, np: items.length, bm: 0 }],
  }
}

export function syntheticLottie(options: SyntheticOptions): Record<string, unknown> {
  const full = { layers: 24, vertices: 12, frameRate: 60, frames: 180, size: 512, ...options }
  const random = rng(full.seed)
  return {
    v: '5.7.4',
    fr: full.frameRate,
    ip: 0,
    op: full.frames,
    w: full.size,
    h: full.size,
    nm: `synthetic ${full.seed}`,
    ddd: 0,
    assets: [],
    layers: Array.from({ length: full.layers }, (_, index) => shapeLayer(random, index, full)),
  }
}
