export interface BenchParams {
  mode: 'raw' | 'managed' | 'cost'
  renderer: 'svg' | 'canvas' | 'html'
  count: number
  distinct: number
  layers: number
  size: number
  offscreen: boolean
  share: boolean
  engine: 'auto' | 'worker' | 'main'
}

export function readParams(search: string): BenchParams {
  const query = new URLSearchParams(search)
  const int = (name: string, fallback: number) => {
    const value = Number.parseInt(query.get(name) ?? '', 10)
    return Number.isFinite(value) && value >= 0 ? value : fallback
  }
  const renderer = query.get('renderer')
  return {
    mode: query.get('mode') === 'managed' ? 'managed' : query.get('mode') === 'cost' ? 'cost' : 'raw',
    renderer: renderer === 'canvas' || renderer === 'html' ? renderer : 'svg',
    count: int('count', 20),
    distinct: Math.max(1, int('distinct', 20)),
    layers: int('layers', 24),
    size: int('size', 160),
    offscreen: query.get('offscreen') === '1',
    share: query.get('share') !== '0',
    engine: query.get('engine') === 'main' ? 'main' : query.get('engine') === 'worker' ? 'worker' : 'auto',
  }
}
