// TG-202: the chat session feeds `message_views_updated` frames into the channel store.
import { expect, test } from 'bun:test'
import { harness } from '../../../../test/chatSessionHarness'

test('message_views_updated frames reach the channel store', async () => {
  const { session, sockets, stores, online } = harness()
  await online()
  sockets[0]!.receive({
    type: 'message_views_updated',
    views: [
      { message_id: 'p1', views: 10 },
      { message_id: 'p2', views: 3 },
    ],
  })
  expect(stores.channel.getState().views).toEqual({ p1: 10, p2: 3 })
  session.stop()
})
