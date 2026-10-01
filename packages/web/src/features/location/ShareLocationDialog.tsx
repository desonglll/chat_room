/**
 * TG-407: «位置» in the attach menu. Reads the device position once, says plainly how precise
 * it is and who will see it (the privacy notice), and sends it as a static location or starts a
 * live share for 15 minutes / 1 hour / 8 hours. «模糊位置» rounds it to about 1 km first.
 *
 * TG-1301: when the position cannot be read (plain http on a LAN address, a denied permission,
 * no fix) the reason stays as a secondary line and the user picks the place on the map instead;
 * that sends a static location. Live location needs the device position, so it is not offered.
 */
import { lazy, Suspense, useEffect, useState } from 'react'
import type { LocationPointInput } from '@tg/core'
import { approximatePoint, LIVE_LOCATION_PERIODS } from '@tg/core'
import { Button, Modal, Toggle } from '@tg/ui'
import { activeTopicId } from '../forum/activeTopic'
import { locationsApi } from './locationApi'
import { startSharing } from './liveSharing'
import { locationBlockedReason, locationFailureReason } from './locationAccess'
import { t } from '../../i18n/index'

// TG-1301: Leaflet loads only when the device position is unavailable.
const LocationPicker = lazy(() => import('./LocationPicker').then((module) => ({ default: module.LocationPicker })))

const PERIOD_LABEL: Record<number, string> = {
  get 900() {
    return t('w.location.cfc1d0')
  },
  get 3600() {
    return t('w.location.c8fb1c')
  },
  get 28800() {
    return t('w.location.759ce5')
  },
}

type Fix = { state: 'locating' } | { state: 'failed'; reason: string } | { state: 'ready'; point: LocationPointInput }

export default function ShareLocationDialog({ chatId, onClose }: { chatId: string; onClose(): void }) {
  const [fix, setFix] = useState<Fix>({ state: 'locating' })
  const [approximate, setApproximate] = useState(false)
  const [error, setError] = useState('')
  const [picked, setPicked] = useState<LocationPointInput | null>(null)

  useEffect(() => {
    const blocked = locationBlockedReason()
    if (blocked) {
      setFix({ state: 'failed', reason: blocked })
      return
    }
    const geolocation = globalThis.navigator.geolocation
    geolocation.getCurrentPosition(
      (position) =>
        setFix({
          state: 'ready',
          point: {
            latitude: position.coords.latitude,
            longitude: position.coords.longitude,
            accuracy_m: position.coords.accuracy,
          },
        }),
      (failure) => setFix({ state: 'failed', reason: locationFailureReason(failure) }),
      { enableHighAccuracy: true, timeout: 15_000 },
    )
  }, [])

  const send = (liveSeconds?: number) => {
    const point =
      fix.state === 'ready' ? (approximate ? approximatePoint(fix.point) : fix.point) : liveSeconds ? null : picked
    if (!point) return
    const topic = activeTopicId(chatId)
    locationsApi
      .send(chatId, {
        ...point,
        ...(liveSeconds ? { live_seconds: liveSeconds } : {}),
        ...(topic ? { topic_id: topic } : {}),
      })
      .then(
        (message) => {
          if (liveSeconds && message.location?.live_until) {
            startSharing(
              { chatId, messageId: message.id, until: Date.parse(message.location.live_until), approximate },
              locationsApi,
            )
          }
          onClose()
        },
        () => setError(t('w.location.033789')),
      )
  }

  const accuracy = fix.state === 'ready' ? Math.round(approximate ? 1000 : (fix.point.accuracy_m ?? 0)) : 0
  return (
    <Modal open onClose={onClose} title={t('w.location.1184ec')}>
      <div className="tg-location-share">
        {fix.state === 'locating' ? <p>{t('w.location.cd8727')}</p> : null}
        {fix.state === 'failed' ? (
          <>
            <p className="tg-location-share__reason" role="status">
              {fix.reason}
            </p>
            <p className="tg-location-share__privacy">{t('w.location.175289')}</p>
            <Suspense fallback={<div className="tg-location-picker" />}>
              <LocationPicker onChange={setPicked} />
            </Suspense>
            <Button fullWidth disabled={!picked} onClick={() => send()}>
              {t('w.location.704cff')}
            </Button>
            <p className="tg-location-share__label">{t('w.location.466085')}</p>
          </>
        ) : null}
        {fix.state === 'ready' ? (
          <>
            <p className="tg-location-share__privacy">
              {t('w.location.2cfb94')}
              {approximate ? t('w.location.8bdf2f') : t('w.location.be955b')}
              {t('w.location.ce34f2')} {accuracy} {t('w.location.cb755e')}
            </p>
            <Toggle label={t('w.location.28849a')} checked={approximate} onCheckedChange={setApproximate} />
            <Button fullWidth onClick={() => send()}>
              {t('w.location.8195a1')}
            </Button>
            <p className="tg-location-share__label">{t('w.location.c9ec06')}</p>
            <div className="tg-location-share__periods">
              {LIVE_LOCATION_PERIODS.map((seconds) => (
                <Button key={seconds} variant="tonal" onClick={() => send(seconds)}>
                  {PERIOD_LABEL[seconds]}
                </Button>
              ))}
            </div>
          </>
        ) : null}
        {error ? <p role="alert">{error}</p> : null}
      </div>
    </Modal>
  )
}
