/**
 * One requestAnimationFrame loop for every playing sticker, with a main-thread budget.
 *
 * lottie-web's own loop renders every playing animation on every frame no matter how long that
 * takes, so twenty stickers on a slow machine turn the whole page into a slideshow (measured:
 * docs/devlog/TG-301.md "Benchmark"). Here each tick advances every clock (cheap), then draws
 * the players whose frame changed, round-robin from where the previous tick stopped, until
 * `budgetMs` of main-thread time is used. Players left undrawn stay due and are served first
 * next tick. Under load stickers lose frames; the page — scrolling, typing — does not.
 *
 * The loop stops entirely when nothing is playing: off-screen and paused cost zero.
 */

export interface TickPlayer {
  /** Advance by `elapsedMs`; return true when a new frame should be drawn. */
  advance(elapsedMs: number, fpsCap: number): boolean
  draw(): void
}

export interface TickerHost {
  requestFrame(callback: (now: number) => void): number
  cancelFrame(handle: number): void
  now(): number
}

export interface TickerOptions {
  budgetMs: number
  /** Frame-rate cap as a function of how many players are running. */
  fpsCap(playing: number): number
}

export interface TickerStats {
  ticks: number
  draws: number
  /** Draws skipped because the budget ran out. */
  deferred: number
}

export interface FrameTicker {
  add(player: TickPlayer): void
  remove(player: TickPlayer): void
  readonly size: number
  readonly running: boolean
  stats(): TickerStats
}

export function createFrameTicker(host: TickerHost, options: TickerOptions): FrameTicker {
  const players: TickPlayer[] = []
  const due = new Set<TickPlayer>()
  let cursor = 0
  let handle: number | null = null
  let last = 0
  const stats: TickerStats = { ticks: 0, draws: 0, deferred: 0 }

  const tick = (now: number) => {
    handle = null
    const elapsed = now - last
    last = now
    stats.ticks += 1
    const cap = options.fpsCap(players.length)
    for (const player of players) if (player.advance(elapsed, cap)) due.add(player)
    const started = host.now()
    const count = players.length
    for (let visited = 0; visited < count && due.size > 0; visited += 1) {
      const player = players[(cursor + visited) % count]
      if (!player || !due.has(player)) continue
      if (host.now() - started >= options.budgetMs) {
        stats.deferred += due.size
        cursor = (cursor + visited) % count
        schedule()
        return
      }
      due.delete(player)
      player.draw()
      stats.draws += 1
    }
    cursor = count === 0 ? 0 : (cursor + 1) % count
    schedule()
  }

  const schedule = () => {
    if (handle === null && players.length > 0) handle = host.requestFrame(tick)
  }

  return {
    add(player) {
      if (players.includes(player)) return
      if (players.length === 0) last = host.now()
      players.push(player)
      schedule()
    },
    remove(player) {
      const index = players.indexOf(player)
      if (index === -1) return
      players.splice(index, 1)
      due.delete(player)
      if (index < cursor) cursor -= 1
      if (players.length === 0 && handle !== null) {
        host.cancelFrame(handle)
        handle = null
      }
    },
    get size() {
      return players.length
    },
    get running() {
      return handle !== null
    },
    stats: () => ({ ...stats }),
  }
}
