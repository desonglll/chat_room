/**
 * TG-407: the full map for one location message, merged with every location being shared live
 * in the chat right now (Telegram's shared live map). The sender can stop their live share here.
 */
import { useEffect, useState } from 'react'
import type { LiveLocationEntry, MessageLocation } from '@tg/core'
import { isLiveLocation, liveRemaining } from '@tg/core'
import { Button } from '@tg/ui'
import { locationsApi, osmLink } from './locationApi'
import { LocationMap, type MapPoint } from './LocationMap'
import { stopSharing } from './liveSharing'

export function LocationSheetBody({
  chatId,
  messageId,
  sender,
  location,
  mine,
}: {
  chatId: string
  messageId: string
  sender: string
  location: MessageLocation
  mine: boolean
}) {
  const [others, setOthers] = useState<LiveLocationEntry[]>([])
  const [stopping, setStopping] = useState(false)
  const now = Date.now()
  const live = isLiveLocation(location, now)

  useEffect(() => {
    let cancelled = false
    locationsApi.live(chatId).then(
      (entries) => {
        if (!cancelled) setOthers(entries.filter((entry) => entry.message_id !== messageId))
      },
      () => undefined,
    )
    return () => {
      cancelled = true
    }
  }, [chatId, messageId])

  const points: MapPoint[] = [
    {
      id: messageId,
      latitude: location.latitude,
      longitude: location.longitude,
      label: sender,
      accuracy_m: location.accuracy_m,
    },
    ...others.map((entry) => ({
      id: entry.message_id,
      latitude: entry.location.latitude,
      longitude: entry.location.longitude,
      label: entry.sender,
      accuracy_m: entry.location.accuracy_m,
    })),
  ]

  return (
    <div className="tg-location-sheet">
      <LocationMap points={points} interactive className="tg-location-map tg-location-map--full" />
      <ul className="tg-location-sheet__list">
        <li>
          <strong>{location.title || sender}</strong>
          {location.address ? <span> · {location.address}</span> : null}
          {live ? <span className="tg-location__live"> · {liveRemaining(location, now)}</span> : null}
        </li>
        {others.map((entry) => (
          <li key={entry.message_id}>
            {entry.sender}
            <span className="tg-location__live"> · {liveRemaining(entry.location, now)}</span>
          </li>
        ))}
      </ul>
      <div className="tg-location-sheet__actions">
        <a href={osmLink(location.latitude, location.longitude)} target="_blank" rel="noreferrer noopener">
          在地图中打开
        </a>
        {mine && live ? (
          <Button
            variant="danger"
            size="sm"
            loading={stopping}
            onClick={() => {
              setStopping(true)
              void stopSharing(chatId, messageId, locationsApi).finally(() => setStopping(false))
            }}
          >
            停止共享
          </Button>
        ) : null}
      </div>
    </div>
  )
}
