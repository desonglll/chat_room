import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { DiscussionApi } from '@tg/core'
import { CommentsPanel } from '../comments/CommentsPanel'
import { commentsLabel, replyTarget } from '../comments/commentsModel'
import { DiscussionLinkEditor } from '../comments/DiscussionLinkEditor'
import { PostCommentsEntry } from '../comments/PostCommentsEntry'
import { makeMessage } from '../../message/fixtures/bubbleFixtures'

const api = {} as DiscussionApi

describe('TG-203 channel comments (web)', () => {
  test('the entry counts comments, or invites the first one', () => {
    expect(commentsLabel(0)).toBe('发表评论')
    expect(commentsLabel(12)).toBe('12 条评论')
  })

  test('a reply names the comment it answers, never the post itself', () => {
    const byId = new Map([['c1', { sender: 'bob' }]])
    expect(replyTarget('c1', 'root', byId)).toBe('bob')
    expect(replyTarget('root', 'root', byId)).toBe('')
    expect(replyTarget(undefined, 'root', byId)).toBe('')
  })

  test('posts without a thread get no entry; the panel starts by loading', () => {
    expect(renderToStaticMarkup(<PostCommentsEntry message={makeMessage()} />)).toBe('')
    expect(renderToStaticMarkup(<CommentsPanel channelId="c" postId="p" api={api} />)).toContain('正在加载评论')
  })

  test('only a channel administrator sees the discussion-group setting', () => {
    expect(
      renderToStaticMarkup(
        <DiscussionLinkEditor chatId="c" chatType="group" myPermissions={['chat.info']} api={api} />,
      ),
    ).toBe('')
    expect(
      renderToStaticMarkup(<DiscussionLinkEditor chatId="c" chatType="channel" myPermissions={[]} api={api} />),
    ).toBe('')
    const html = renderToStaticMarkup(
      <DiscussionLinkEditor chatId="c" chatType="channel" myPermissions={['chat.info']} api={api} />,
    )
    expect(html).toContain('讨论组')
    expect(html).toContain('选择讨论组')
  })
})
