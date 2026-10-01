/**
 * TG-407: «位置» in the attach menu. Reads the device position once, says plainly how precise
 * it is and who will see it (the privacy notice), and sends it as a static location or starts a
 * live share for 15 minutes / 1 hour / 8 hours. «模糊位置» rounds it to about 1 km first.
 */
import { useEffect, useState } from 'react'
import type { LocationPointInput } from '@tg/core'
import { approximatePoint, LIVE_LOCATION_PERIODS } from '@tg/core'
import { Button, Modal, Toggle } from '@tg/ui'
import { activeTopicId } from '../forum/activeTopic'
import { locationsApi } from './locationApi'
import { startSharing } from './liveSharing'

const PERIOD_LABEL: Record<number, string> = { 900: '15 分钟', 3600: '1 小时', 28800: '8 小时' }

type Fix = { state: 'locating' } | { state: 'failed'; reason: string } | { state: 'ready'; point: LocationPointInput }

export default function ShareLocationDialog({ chatId, onClose }: { chatId: string; onClose(): void }) {
  const [fix, setFix] = useState<Fix>({ state: 'locating' })
  const [approximate, setApproximate] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    const geolocation = globalThis.navigator?.geolocation
    if (!geolocation) {
      setFix({ state: 'failed', reason: '此浏览器不支持定位' })
      return
    }
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
      (failure) => setFix({ state: 'failed', reason: failure.code === 1 ? '未获得定位权限' : '无法获取当前位置' }),
      { enableHighAccuracy: true, timeout: 15_000 },
    )
  }, [])

  const send = (liveSeconds?: number) => {
    if (fix.state !== 'ready') return
    const point = approximate ? approximatePoint(fix.point) : fix.point
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
        () => setError('无法在这个会话中发送位置'),
      )
  }

  const accuracy = fix.state === 'ready' ? Math.round(approximate ? 1000 : (fix.point.accuracy_m ?? 0)) : 0
  return (
    <Modal open onClose={onClose} title="发送位置">
      <div className="tg-location-share">
        {fix.state === 'locating' ? <p>正在获取当前位置…</p> : null}
        {fix.state === 'failed' ? <p role="alert">{fix.reason}</p> : null}
        {fix.state === 'ready' ? (
          <>
            <p className="tg-location-share__privacy">
              这个会话里的所有成员都能看到{approximate ? '大致' : '精确'}位置（误差约 {accuracy} 米）。
              实时位置只保留最新一点，结束后不再更新，也无法回看轨迹。
            </p>
            <Toggle label="模糊位置（约 1 公里）" checked={approximate} onCheckedChange={setApproximate} />
            <Button fullWidth onClick={() => send()}>
              发送当前位置
            </Button>
            <p className="tg-location-share__label">共享实时位置</p>
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
