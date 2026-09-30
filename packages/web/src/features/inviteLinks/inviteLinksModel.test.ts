import { describe, expect, test } from 'bun:test'
import {
  canChangeLink,
  draftFromLink,
  draftToInput,
  emptyDraft,
  linkName,
  linkSummary,
  partitionLinks,
} from './inviteLinksModel'
import { sampleLink } from './inviteLinksFixtures'

const now = new Date('2026-10-01T10:00:00Z')

describe('invite link drafts', () => {
  test('presets become an absolute expiry, a blank limit is unlimited', () => {
    const input = draftToInput({ ...emptyDraft(), title: ' 渠道A ', expiry: 'day' }, now)
    expect(input).toEqual({
      title: '渠道A',
      expires_at: '2026-10-02T10:00:00.000Z',
      usage_limit: null,
      requires_approval: false,
    })
  })

  test('limits are validated and dropped for approval links', () => {
    expect(draftToInput({ ...emptyDraft(), limit: '0' }, now)).toBe('人数上限为 1 – 99999')
    expect(draftToInput({ ...emptyDraft(), limit: '2.5' }, now)).toBe('人数上限必须是整数')
    expect(draftToInput({ ...emptyDraft(), limit: '10' }, now)).toMatchObject({ usage_limit: 10 })
    expect(draftToInput({ ...emptyDraft(), limit: '10', requiresApproval: true }, now)).toMatchObject({
      usage_limit: null,
      requires_approval: true,
    })
    expect(draftToInput({ ...emptyDraft(), title: 'x'.repeat(33) }, now)).toBe('名称最多 32 个字符')
  })

  test('editing keeps an existing expiry until it has passed', () => {
    const draft = draftFromLink(sampleLink('a', { expires_at: '2026-10-05T00:00:00Z', usage_limit: 7 }))
    expect(draft).toMatchObject({ expiry: 'keep', limit: '7' })
    expect(draftToInput(draft, now)).toMatchObject({ expires_at: '2026-10-05T00:00:00Z', usage_limit: 7 })
    expect(draftToInput(draft, new Date('2026-10-06T00:00:00Z'))).toBe('有效期已过，请重新选择')
  })
})

describe('invite link views', () => {
  test('summary says usage, pending, and why a link stopped working', () => {
    expect(linkSummary(sampleLink('a', { usage_count: 3, usage_limit: 5 }))).toBe('3/5 人已加入')
    expect(linkSummary(sampleLink('a', { pending_count: 2, requires_approval: true }))).toBe(
      '0 人已加入 · 2 个待审核 · 需审核',
    )
    expect(linkSummary(sampleLink('a', { state: 'revoked' }))).toContain('已撤销')
    expect(linkSummary(sampleLink('a', { state: 'limit_reached' }))).toContain('名额已满')
    expect(linkName(sampleLink('a', { is_primary: true }))).toBe('主邀请链接')
    expect(linkName(sampleLink('a', { title: '渠道' }))).toBe('渠道')
  })

  test('partition keeps server order and pulls out the live primary', () => {
    const links = [
      sampleLink('p', { is_primary: true }),
      sampleLink('b'),
      sampleLink('old-p', { is_primary: true, state: 'revoked' }),
      sampleLink('r', { state: 'revoked' }),
    ]
    const { primary, live, revoked } = partitionLinks(links)
    expect(primary?.id).toBe('p')
    expect(live.map((link) => link.id)).toEqual(['b'])
    expect(revoked.map((link) => link.id)).toEqual(['old-p', 'r'])
  })

  test('another administrator’s link needs the manage-others right', () => {
    const theirs = sampleLink('t', { creator_id: 'someone' })
    expect(canChangeLink(theirs, 'me', false)).toBe(false)
    expect(canChangeLink(theirs, 'me', true)).toBe(true)
    expect(canChangeLink(sampleLink('m', { creator_id: 'me' }), 'me', false)).toBe(true)
    expect(canChangeLink(sampleLink('p', { is_primary: true, creator_id: 'x' }), 'me', false)).toBe(true)
  })
})
