/**
 * The sticker render manager: every `<AnimatedSticker>` on the page is a view registered here.
 *
 * Per view it runs source → parse (shared cache) → first-frame snapshot (shared cache) →
 * static poster, and then lets `admitPlayback` decide whether the view animates. Animating
 * views join a `StickerGroup` that the budgeted `FrameTicker` drives. Views that are
 * off-screen, in a hidden tab, or over the cap stop costing CPU; reduced motion keeps every
 * view on its static first frame.
 *
 * Framework-free and DOM-free apart from the injected services, so it is unit-tested with
 * fakes and can back the WebP/WebM sticker kinds TG-306 adds behind the same entry.
 */
import type { LoadedSticker, StickerEngine } from './engineTypes'
import { createFrameTicker, type FrameTicker } from './frameTicker'
import type {
  ManagerPolicy,
  ManagerServices,
  StickerRenderManager,
  StickerViewOptions,
  StickerViewState,
} from './managerTypes'
import { admitPlayback } from './playbackPolicy'
import { createRefCountedCache } from './refCountedCache'
import { StickerGroup, type GroupMember } from './stickerGroup'
import { sourceKey } from './stickerSource'

export type * from './managerTypes'

interface ViewRecord extends GroupMember {
  readonly id: number
  readonly options: StickerViewOptions
  readonly key: string
  readonly pixelSize: number
  loaded: LoadedSticker | null
  state: StickerViewState
  visible: boolean
  visibleSince: number
  replayRequested: boolean
  finished: boolean
  playing: boolean
  group: StickerGroup | null
  joining: boolean
  destroyed: boolean
  holdsParse: boolean
  holdsPoster: boolean
}

export function createStickerRenderManager(services: ManagerServices, policy: ManagerPolicy): StickerRenderManager {
  const views = new Map<number, ViewRecord>()
  const groups = new Map<string, StickerGroup>()
  const pendingGroups = new Map<string, Promise<StickerGroup>>()
  const idleTimers = new Map<string, ReturnType<typeof setTimeout>>()
  const ticker: FrameTicker = createFrameTicker(services.tickerHost, {
    budgetMs: policy.budgetMs,
    fpsCap: policy.fpsCap,
  })
  let engine: StickerEngine | null = null
  const engineReady = () =>
    services.loadEngine().then((loaded) => {
      engine = loaded
      return loaded
    })
  // Parsed documents live in the engine (possibly in a worker); this cache holds the handles
  // and tells the engine when an unreferenced one is evicted.
  const parsed = createRefCountedCache<LoadedSticker>({
    maxIdleCost: 64,
    cost: () => 1,
    onEvict: (loaded) => engine?.unload(loaded.key),
  })
  const posters = createRefCountedCache<string>({
    maxIdleCost: 256,
    cost: () => 1,
    onEvict: (url) => services.objectUrls.revoke(url),
  })
  let nextId = 1
  let visibleCounter = 0
  let planQueued = false

  const setState = (view: ViewRecord, patch: Partial<StickerViewState>) => {
    if (view.destroyed) return
    view.state = { ...view.state, ...patch }
    view.options.onState(view.state)
  }

  // Looping autoplay views share one clock per sticker and size; a play-once view (non-looping,
  // or a replay the user asked for) needs its own clock, hence its own group.
  const groupKeyOf = (view: ViewRecord) =>
    view.options.loop && !view.replayRequested
      ? `${view.key}@${view.pixelSize}`
      : `${view.key}@${view.pixelSize}#${view.id}`
  const wantsMotion = (view: ViewRecord) =>
    view.replayRequested || (view.options.autoplay && (view.options.loop || !view.finished))

  const schedulePlan = () => {
    if (planQueued) return
    planQueued = true
    queueMicrotask(plan)
  }

  function plan() {
    planQueued = false
    const reducedMotion = services.signals.reducedMotion()
    const candidates = [...views.values()]
      .filter((view) => view.loaded !== null)
      .map((view) => ({
        id: view.id,
        group: groupKeyOf(view),
        visible: view.visible,
        wantsMotion: wantsMotion(view),
        playing: view.playing,
        visibleSince: view.visibleSince,
      }))
    const admitted = admitPlayback(candidates, {
      hidden: services.signals.hidden(),
      reducedMotion,
      maxGroups: policy.maxGroups,
    })
    for (const view of views.values()) {
      view.playing = admitted.has(view.id)
      if (view.group && (reducedMotion || (view.playing && view.group.key !== groupKeyOf(view)))) leaveGroup(view)
      if (view.playing && !view.group && !view.joining) joinGroup(view)
    }
    for (const [key, group] of groups) {
      const running = [...group.members].some((member) => (member as ViewRecord).playing) && !group.ended
      if (running) {
        ticker.add(group)
        const timer = idleTimers.get(key)
        if (timer !== undefined) clearTimeout(timer)
        idleTimers.delete(key)
      } else {
        ticker.remove(group)
        if (!idleTimers.has(key))
          idleTimers.set(
            key,
            setTimeout(() => releaseIdle(key), policy.idleReleaseMs),
          )
      }
    }
  }

  function releaseIdle(key: string) {
    idleTimers.delete(key)
    const group = groups.get(key)
    if (!group || [...group.members].some((member) => (member as ViewRecord).playing)) return
    // A hidden tab pauses everything; releasing then would only rebuild it all on return.
    if (services.signals.hidden()) {
      idleTimers.set(
        key,
        setTimeout(() => releaseIdle(key), policy.idleReleaseMs),
      )
      return
    }
    for (const member of [...group.members]) leaveGroup(member as ViewRecord)
    if (groups.get(key) === group) dropGroup(group)
  }

  function joinGroup(view: ViewRecord) {
    if (!view.loaded) return
    const key = groupKeyOf(view)
    view.joining = true
    const existing = groups.get(key)
    let pending = existing ? Promise.resolve(existing) : pendingGroups.get(key)
    if (!pending) {
      pending = engineReady()
        .then((ready) => ready.createUnit(view.key, view.pixelSize))
        .then((unit) => {
          const group = new StickerGroup(key, unit, !key.includes('#'), services.blit)
          groups.set(key, group)
          return group
        })
        .finally(() => pendingGroups.delete(key))
      pendingGroups.set(key, pending)
    }
    pending.then(
      (group) => {
        view.joining = false
        // No longer wanted: an empty group is collected by the idle timer `plan` arms, which
        // also keeps it alive for another view that is waiting on the same unit.
        if (view.destroyed || !view.playing || services.signals.reducedMotion() || !groups.has(group.key)) {
          schedulePlan()
          return
        }
        view.group = group
        group.join(view)
        schedulePlan()
      },
      (error: unknown) => {
        view.joining = false
        view.options.onError?.(error)
      },
    )
  }

  function leaveGroup(view: ViewRecord) {
    const group = view.group
    if (!group) return
    view.group = null
    group.leave(view)
    if (!view.destroyed) setState(view, { phase: view.state.poster ? 'static' : 'loading' })
    if (group.members.size === 0) dropGroup(group)
  }

  function dropGroup(group: StickerGroup) {
    ticker.remove(group)
    groups.delete(group.key)
    const timer = idleTimers.get(group.key)
    if (timer !== undefined) clearTimeout(timer)
    idleTimers.delete(group.key)
    group.destroy()
  }

  async function prepare(view: ViewRecord) {
    const { source } = view.options
    try {
      view.holdsParse = true
      const loaded = await parsed.acquire(view.key, async () => {
        const [ready, bytes] = await Promise.all([
          engineReady(),
          'url' in source ? services.fetchBytes(source.url) : Promise.resolve(source.bytes),
        ])
        return ready.load(view.key, bytes)
      })
      if (view.destroyed) return
      view.loaded = loaded
      schedulePlan()
      view.holdsPoster = true
      const poster = await posters.acquire(`${view.key}@${view.pixelSize}`, async () => {
        const ready = await engineReady()
        return services.objectUrls.create(await ready.snapshot(view.key, view.pixelSize))
      })
      if (view.destroyed) return
      setState(view, { poster, phase: view.state.phase === 'live' ? 'live' : 'static' })
    } catch (error) {
      if (view.destroyed) return
      setState(view, { phase: 'error' })
      view.options.onError?.(error)
    }
  }

  services.signals.subscribe(schedulePlan)

  return {
    attach(options) {
      const pixelSize = Math.max(1, Math.round(options.size * services.signals.pixelRatio()))
      options.canvas.width = pixelSize
      options.canvas.height = pixelSize
      const view: ViewRecord = {
        id: nextId++,
        options,
        key: sourceKey(options.source),
        pixelSize,
        loaded: null,
        state: { phase: 'loading', poster: null },
        visible: false,
        visibleSince: 0,
        replayRequested: false,
        finished: false,
        playing: false,
        group: null,
        joining: false,
        destroyed: false,
        holdsParse: false,
        holdsPoster: false,
        canvas: options.canvas,
        onFirstFrame: () => setState(view, { phase: 'live' }),
        onEnded: () => {
          view.finished = true
          view.replayRequested = false
          schedulePlan()
        },
      }
      views.set(view.id, view)
      options.onState(view.state)
      const unobserve = services.viewport.observe(options.element, (visible) => {
        if (visible && !view.visible) view.visibleSince = ++visibleCounter
        view.visible = visible
        schedulePlan()
      })
      void prepare(view)
      return {
        replay() {
          if (view.destroyed) return
          view.finished = false
          view.replayRequested = true
          // A private group restarts in place; a shared one is left by `plan` for a private one.
          if (view.group?.key === groupKeyOf(view)) view.group.restart()
          schedulePlan()
        },
        destroy() {
          if (view.destroyed) return
          view.destroyed = true
          unobserve()
          leaveGroup(view)
          views.delete(view.id)
          if (view.holdsParse) parsed.release(view.key)
          if (view.holdsPoster) posters.release(`${view.key}@${view.pixelSize}`)
          schedulePlan()
        },
      }
    },
    stats() {
      const all = [...views.values()]
      return {
        ...ticker.stats(),
        views: all.length,
        liveViews: all.filter((view) => view.state.phase === 'live').length,
        playingViews: all.filter((view) => view.playing).length,
        groups: groups.size,
        runningGroups: ticker.size,
        parsed: parsed.size,
      }
    },
  }
}
