/**
 * TG-105 root component. Mount once at the app root; it renders nothing until
 * `openMediaViewer()` is called. The open viewer (and `motion`, and plyr behind it) is a
 * lazy chunk, fetched the moment a pointer goes down on a bubble thumbnail so that it is
 * normally ready by the time the click opens it.
 */
import { lazy, Suspense, useEffect, useState } from 'react'
import { useStore } from 'zustand/react'
import { authStore, selectToken } from '@tg/core'
import { apiClient } from '../../app/client'
import type { MediaViewerRequest } from './mediaViewerStore'
import { mediaViewerStore } from './mediaViewerStore'
import { createChatMediaFetcher } from './chatMediaSource'
import type { FetchMediaPage } from './mediaPager'
import type { MediaViewerActions, MediaViewerProps } from './types'

const loadSurface = () => import('./ViewerSurface')
const ViewerSurface = lazy(loadSurface)

const NO_ACTIONS: MediaViewerActions = {}

function defaultFetcher(chatId: string): FetchMediaPage {
  return createChatMediaFetcher(chatId, { client: apiClient, token: () => selectToken(authStore.getState()) })
}

/** Warm the chunk on the first press of any bubble thumbnail. */
function usePreloadOnPress() {
  useEffect(() => {
    const onPress = (event: PointerEvent) => {
      if (event.target instanceof Element && event.target.closest('.tg-bubble__media')) {
        void loadSurface()
        document.removeEventListener('pointerdown', onPress, true)
      }
    }
    document.addEventListener('pointerdown', onPress, true)
    return () => document.removeEventListener('pointerdown', onPress, true)
  }, [])
}

export function MediaViewer({ actions = NO_ACTIONS, createFetcher = defaultFetcher }: MediaViewerProps) {
  const request = useStore(mediaViewerStore, (state) => state.request)
  usePreloadOnPress()
  if (!request) return null
  return (
    <Suspense fallback={null}>
      <OpenViewer key={request.openId} request={request} actions={actions} createFetcher={createFetcher} />
    </Suspense>
  )
}

function OpenViewer({
  request,
  actions,
  createFetcher,
}: {
  request: MediaViewerRequest
  actions: MediaViewerActions
  createFetcher: (chatId: string) => FetchMediaPage
}) {
  const [fetchPage] = useState(() => createFetcher(request.chatId))
  return (
    <ViewerSurface
      request={request}
      actions={actions}
      fetchPage={fetchPage}
      onClosed={() => {
        // A newer open may already have replaced this one; never close that.
        if (mediaViewerStore.getState().request?.openId === request.openId) mediaViewerStore.getState().close()
      }}
    />
  )
}
