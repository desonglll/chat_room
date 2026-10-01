/**
 * TG-407: a location bubble — a small map, the venue, and for a live location the time left
 * (or «实时位置已结束»). Tapping opens the full shared map.
 */
import { useState } from 'react'
import { isLiveLocation, liveRemaining, uiStore } from '@tg/core'
import { Sheet } from '@tg/ui'
import { useStore } from 'zustand/react'
import type { MessageContentProps } from '../message'
import { LocationMap } from './LocationMap'
import { LocationSheetBody } from './LocationSheet'
import { effectiveLocation, liveLocationStore } from './liveLocationStore'
import { t } from '../../i18n/index'

export function LocationContent({ message, ctx, metaSpacer }: MessageContentProps) {
  const held = useStore(liveLocationStore, (state) => state.points[message.message_id])
  const chatId = useStore(uiStore, (state) => state.activeChatId)
  const [open, setOpen] = useState(false)
  if (!message.location) return null
  const location = effectiveLocation(message.location, held)
  const now = Date.now()
  const live = location.live_until !== undefined
  const label = location.title || (live ? t('w.location.3b5fd7') : t('w.location.88c344'))
  return (
    <div className="tg-location">
      <button
        type="button"
        className="tg-location__open"
        aria-label={t('w.location.1813aa', label)}
        onClick={() => setOpen(true)}
      >
        <LocationMap
          points={[
            {
              id: message.message_id,
              latitude: location.latitude,
              longitude: location.longitude,
              label: message.sender,
              accuracy_m: location.accuracy_m,
            },
          ]}
        />
      </button>
      <div className="tg-location__text">
        <span className="tg-location__title">{label}</span>
        {location.address ? <span className="tg-location__address">{location.address}</span> : null}
        {live ? (
          <span className="tg-location__live">
            {isLiveLocation(location, now) ? liveRemaining(location, now) : t('w.location.26fd6a')}
          </span>
        ) : null}
        {metaSpacer}
      </div>
      <Sheet open={open} side="right" title={label} onClose={() => setOpen(false)}>
        {open && chatId ? (
          <LocationSheetBody
            chatId={chatId}
            messageId={message.message_id}
            sender={message.sender}
            location={location}
            mine={ctx.isOutgoing}
          />
        ) : null}
      </Sheet>
    </div>
  )
}
