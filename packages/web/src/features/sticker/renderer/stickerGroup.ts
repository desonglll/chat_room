/**
 * A render group: one sticker, at one pixel size, on one clock. It owns the engine's render
 * unit and copies each new frame to every member view's canvas — so five copies of a looping
 * sticker cost one render and five `drawImage`s. Play-once views (non-looping, or a replay the
 * user asked for) get a group of their own, because they need their own clock.
 *
 * At most one frame request is in flight; the group never re-requests the frame it already
 * shows (a joining view or a restart on frame 0 is served from the last image).
 */
import type { FrameImage, RenderUnit } from './engineTypes'
import type { TickPlayer } from './frameTicker'
import { advanceClock, createClock, frameToDraw, restartClock, type PlayerClock } from './playerClock'

export interface GroupMember {
  readonly canvas: HTMLCanvasElement
  /** Called when the member's canvas first receives a frame from this group. */
  onFirstFrame(): void
  onEnded(): void
}

export type Blit = (target: HTMLCanvasElement, image: FrameImage) => void

export class StickerGroup implements TickPlayer {
  readonly members = new Set<GroupMember>()
  private readonly clock: PlayerClock
  private readonly presented = new Set<GroupMember>()
  private image: FrameImage | null = null
  private drawn = -1
  private next = 0
  private requesting = false
  private destroyed = false

  constructor(
    readonly key: string,
    private readonly unit: RenderUnit,
    loop: boolean,
    private readonly blit: Blit,
  ) {
    this.clock = createClock(unit.frames, unit.frameRate, loop)
  }

  get ended(): boolean {
    return this.clock.ended
  }

  join(member: GroupMember): void {
    if (this.members.has(member)) return
    this.members.add(member)
    if (this.image) this.present(member, this.image)
    else this.draw()
  }

  leave(member: GroupMember): void {
    this.members.delete(member)
    this.presented.delete(member)
  }

  restart(): void {
    restartClock(this.clock)
    this.next = 0
    if (this.drawn === 0 && this.image) this.presentAll(this.image)
    else this.draw()
  }

  advance(elapsedMs: number, fpsCap: number): boolean {
    const wasEnded = this.clock.ended
    advanceClock(this.clock, elapsedMs)
    this.next = frameToDraw(this.clock, fpsCap, this.unit.pipelined)
    if (this.clock.ended && !wasEnded) queueMicrotask(() => this.members.forEach((member) => member.onEnded()))
    return this.next !== this.drawn && !this.requesting
  }

  draw(): void {
    if (this.requesting || this.destroyed || (this.next === this.drawn && this.image)) return
    const frame = this.next
    this.requesting = true
    const accepted = this.unit.request(frame, (image) => {
      this.requesting = false
      if (this.destroyed) return
      this.drawn = frame
      this.image = image
      this.presentAll(image)
    })
    if (!accepted) this.requesting = false
  }

  destroy(): void {
    this.destroyed = true
    this.members.clear()
    this.presented.clear()
    this.image = null
    this.unit.destroy()
  }

  private presentAll(image: FrameImage): void {
    for (const member of this.members) this.present(member, image)
  }

  private present(member: GroupMember, image: FrameImage): void {
    this.blit(member.canvas, image)
    if (this.presented.has(member)) return
    this.presented.add(member)
    member.onFirstFrame()
  }
}
