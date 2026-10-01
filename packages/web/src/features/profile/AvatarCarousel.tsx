/**
 * TG-511: the profile photo carousel — every photo, newest first; the owner can set an older
 * one as the main photo or delete one (deleting the main photo promotes the next newest).
 * TG-1204: the owner can also upload a new photo here (there was no upload anywhere).
 */
import { useCallback, useEffect, useState } from 'react'
import { ApiError, authStore } from '@tg/core'
import { Button, IconButton } from '@tg/ui'
import { useStore } from 'zustand/react'
import { profileApi, type AvatarHistoryEntry } from './profileApi'
import { t } from '../../i18n/index'

export function AvatarCarousel({ userId }: { userId: string }) {
  const viewerId = useStore(authStore, (state) => state.session?.user.id ?? '')
  const own = viewerId === userId
  const [photos, setPhotos] = useState<AvatarHistoryEntry[]>([])
  const [index, setIndex] = useState(0)
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState('')

  const reload = useCallback(() => {
    profileApi.avatars(userId).then(
      (list) => {
        setPhotos(list)
        setIndex((current) => Math.min(current, Math.max(0, list.length - 1)))
      },
      () => setPhotos([]),
    )
  }, [userId])
  useEffect(reload, [reload])

  const upload = (file: File) => {
    setBusy(true)
    setFailure('')
    profileApi
      .upload(file)
      .then((user) => {
        authStore.getState().updateUser(user)
        setIndex(0)
        reload()
      })
      .catch((error: unknown) =>
        setFailure(
          error instanceof ApiError && error.status === 413
            ? t('w.profile.uploadTooLarge')
            : t('w.profile.uploadFailed'),
        ),
      )
      .finally(() => setBusy(false))
  }
  const uploader = own ? <AvatarUpload busy={busy} failure={failure} onFile={upload} /> : null

  if (photos.length === 0)
    return (
      <section className="tg-avatar-carousel" aria-label={t('w.profile.4ceeeb')}>
        <p className="tg-avatar-carousel__empty">{t('w.profile.d643e9')}</p>
        {uploader}
      </section>
    )
  const photo = photos[index] ?? photos[0]!
  const act = (run: () => Promise<unknown>) => {
    setBusy(true)
    run()
      .then(reload)
      .finally(() => setBusy(false))
  }
  return (
    <section className="tg-avatar-carousel" aria-label={t('w.profile.4ceeeb')}>
      <div className="tg-avatar-carousel__frame">
        <img
          className="tg-avatar-carousel__image"
          src={photo.url}
          alt={t('w.profile.307551', index + 1, photos.length)}
        />
        <IconButton
          label={t('w.profile.b9f734')}
          className="tg-avatar-carousel__nav"
          data-side="prev"
          disabled={index === 0}
          onClick={() => setIndex(index - 1)}
        >
          ‹
        </IconButton>
        <IconButton
          label={t('w.profile.9a3dc9')}
          className="tg-avatar-carousel__nav"
          data-side="next"
          disabled={index >= photos.length - 1}
          onClick={() => setIndex(index + 1)}
        >
          ›
        </IconButton>
        <span className="tg-avatar-carousel__count">
          {index + 1} / {photos.length}
        </span>
      </div>
      {own ? (
        <div className="tg-avatar-carousel__actions">
          {photo.is_current ? (
            <span className="tg-avatar-carousel__current">{t('w.profile.0c1c90')}</span>
          ) : (
            <Button variant="text" disabled={busy} onClick={() => act(() => profileApi.setMain(photo.id))}>
              {t('w.profile.c7ab2f')}
            </Button>
          )}
          <Button variant="danger" disabled={busy} onClick={() => act(() => profileApi.remove(photo.id))}>
            {t('w.profile.3755f5')}
          </Button>
        </div>
      ) : null}
      {uploader}
    </section>
  )
}

/** The owner's «上传新头像»: a label around a hidden file input, plus the last failure. */
export function AvatarUpload({ busy, failure, onFile }: { busy: boolean; failure: string; onFile(file: File): void }) {
  return (
    <>
      <label className="tg-avatar-carousel__upload" aria-disabled={busy}>
        {t('w.profile.uploadNew')}
        <input
          type="file"
          accept="image/png,image/jpeg,image/gif,image/webp,image/avif"
          hidden
          disabled={busy}
          onChange={(event) => {
            const file = event.target.files?.[0]
            event.target.value = ''
            if (file) onFile(file)
          }}
        />
      </label>
      {failure ? <p role="alert">{failure}</p> : null}
    </>
  )
}
