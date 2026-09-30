/**
 * The app's single presence ticker: browser clock, 5 s regular period (minute-granular
 * last-seen text lags at most that much), plus exact wake-ups at every typing expiry.
 */
import { presenceStore } from '@tg/core'
import { browserClock } from '../../app/platform'
import { createNowTicker } from './nowTicker'
import { nextTypingExpiry } from './presenceText'

export const PRESENCE_TICK_MS = 5_000

export const presenceTicker = createNowTicker({
  clock: browserClock,
  periodMs: PRESENCE_TICK_MS,
  wakeSource: {
    subscribe: (listener) => presenceStore.subscribe(listener),
    nextWakeAt: (now) => nextTypingExpiry(presenceStore.getState(), now),
  },
})
