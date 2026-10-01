/**
 * TG-1203 walkthrough of group and channel administration against a REAL server, with four
 * accounts it registers itself (no seed file). Not part of `bun test`.
 *   BASE_URL=http://127.0.0.1:18203 node packages/web/test/e2e/tg1203.e2e.mjs
 * Covers: group creation → supergroup upgrade, admins and member restrictions, slow mode,
 * invite links (limit, approval, revoke), public username, channel posting / signatures /
 * views / subscribers, the comments discussion group, and edit / leave / delete.
 */
import { mkdirSync } from 'node:fs'
const { chromium } = await import(process.env.PLAYWRIGHT ?? '/tmp/pw/node_modules/playwright/index.mjs')

const BASE = process.env.BASE_URL ?? 'http://127.0.0.1:18203'
const OUT = process.env.SHOTS ?? '/tmp/tg-shots/TG-1203'
mkdirSync(OUT, { recursive: true })
const stamp = Date.now().toString(36)

async function call(token, method, path, body) {
  const r = await fetch(`${BASE}${path}`, {
    method,
    headers: { 'content-type': 'application/json', ...(token ? { authorization: `Bearer ${token}` } : {}) },
    body: body === undefined ? undefined : JSON.stringify(body),
  })
  const text = await r.text()
  return { status: r.status, json: text ? JSON.parse(text) : null }
}
async function api(token, method, path, body) {
  const r = await call(token, method, path, body)
  if (r.status >= 400) throw new Error(`${method} ${path} → ${r.status}`)
  return r.json
}
async function account(name) {
  const username = `${name}_${stamp}${Math.floor(Math.random() * 1000)}`
  const a = await api(null, 'POST', '/api/users/register', { username, password: 'correct-horse-9' })
  return { ...a, username, password: 'correct-horse-9' }
}

const browser = await chromium.launch()
let goneChat = '' // left, then deleted: late requests about it may 403/404
const errors = []
const bad = []
async function page(who, width = 1440) {
  const p = await (await browser.newContext({ viewport: { width, height: 900 } })).newPage()
  p.on('pageerror', (e) => errors.push(`${who.username}: ${e}`))
  if (process.env.CONSOLE)
    p.on('console', (m) => console.log(`  [${who.username.split('_')[0]}] ${m.type()} ${m.text().slice(0, 160)}`))
  p.on('response', (r) => {
    // Answers the walkthrough provokes on purpose: username availability probes, and the
    // full one-use link (410) shown to the second person.
    const path = new URL(r.url()).pathname
    const expected =
      r.url().includes('/check') ||
      (r.status() === 410 && path.startsWith('/api/invite-links/')) ||
      // the banned account's rejoin attempt, and requests about the group after its deletion
      (r.status() === 403 && path.endsWith('/join') && who === dave) ||
      (goneChat !== '' && path.includes(goneChat))
    if (r.status() >= 400 && !expected) bad.push(`${r.status()} ${r.request().method()} ${new URL(r.url()).pathname}`)
  })
  await p.goto(`${BASE}/login`)
  await p.locator('input[name="username"]').fill(who.username)
  await p.locator('input[name="password"]').fill(who.password)
  await p.getByRole('button', { name: '登录', exact: true }).click()
  await p.getByRole('button', { name: '主菜单' }).first().waitFor()
  return p
}

let n = 0
let failed = 0
async function step(p, name, body) {
  n++
  const file = `${OUT}/${String(n).padStart(2, '0')}-${name}`
  try {
    await body()
    await p.waitForTimeout(1300)
    await p.screenshot({ path: `${file}.png` })
    console.log('ok  ', name)
  } catch (e) {
    failed++
    console.log('FAIL', name, String(e).split('\n')[0])
    await p.screenshot({ path: `${file}-FAIL.png` }).catch(() => {})
  }
}

const alice = await account('alice')
const bob = await account('bob')
const carol = await account('carol')
const dave = await account('dave')
const A = await page(alice)
const B = await page(bob)
const C = await page(carol)

/** Type and send; the composer refuses while the socket is still connecting, so retry once. */
async function say(p, text) {
  await p.locator('.tg-compose__input').fill(text)
  for (let attempt = 0; attempt < 3; attempt++) {
    await p.keyboard.press('Enter')
    try {
      await p.locator('.tg-message', { hasText: text }).first().waitFor({ timeout: 2500 })
      return
    } catch {}
  }
  throw new Error(`«${text}» was not sent`)
}

const groupTitle = `治理测试 ${stamp}`
let groupId = ''
const openInfo = async (p) => {
  await p.getByRole('button', { name: '查看会话信息' }).click()
  await p.waitForTimeout(700)
}
const openAdmin = async (p, chatId) => {
  await p.goto(`${BASE}/chat/${chatId}`)
  await p.locator('.tg-compose__input, .tg-channel-footer').first().waitFor()
  await openInfo(p)
  await p
    .getByText(/管理(群组|频道)/)
    .first()
    .click()
  await p.locator('.tg-chatadmin').waitFor()
  await p.waitForTimeout(600)
}

// ---- group creation (UI)
await step(A, 'create-group', async () => {
  await A.getByRole('button', { name: '新建' }).first().click()
  await A.getByRole('menuitem', { name: '新建群组' }).click()
  await A.getByLabel('群组名称').fill(groupTitle)
  await A.getByRole('button', { name: '创建', exact: true }).click()
  await A.waitForURL(/\/chat\/[0-9a-f-]{36}/, { timeout: 6000 })
  groupId = A.url().match(/[0-9a-f-]{36}/)[0]
  await A.locator('.tg-compose__input').waitFor()
})

// ---- invite links: an approval link; bob asks to join and alice approves
let approvalUrl = ''
await step(A, 'invite-links-create-approval', async () => {
  await openAdmin(A, groupId)
  await A.locator('.tg-chatadmin').getByText('邀请链接').first().click()
  await A.locator('.tg-invite').getByText('主邀请链接').first().waitFor()
  await A.getByRole('button', { name: /创建新链接/ }).click()
  await A.getByLabel('链接名称（可选）').fill('审核渠道')
  await A.getByText('需要管理员审核').click()
  await A.getByRole('button', { name: '创建链接' }).click()
  await A.locator('.tg-invite').getByText('审核渠道').first().waitFor({ timeout: 5000 })
  const links = await api(alice.token, 'GET', `/api/chats/${groupId}/invite-links`)
  approvalUrl = `${BASE}/joinchat/${links.links.find((l) => l.title === '审核渠道').token}`
})
await step(B, 'join-request', async () => {
  await B.goto(approvalUrl)
  await B.getByRole('button', { name: '申请加入' }).click()
  await B.getByText('已发送加入申请').first().waitFor({ timeout: 5000 })
})
await step(A, 'approve-request', async () => {
  await openAdmin(A, groupId)
  await A.locator('.tg-chatadmin').getByText('邀请链接').first().click()
  await A.locator('.tg-invite').getByText('审核渠道').first().click()
  await A.getByRole('button', { name: '通过' }).first().click()
  await A.locator('.tg-invite').getByText(bob.username).first().waitFor({ timeout: 5000 })
})
await step(B, 'member-after-approval', async () => {
  await B.goto(`${BASE}/chat/${groupId}`)
  await B.locator('.tg-compose__input').waitFor({ timeout: 6000 })
})

// ---- carol joins by a one-use link; a second person is turned away
let oneUse = ''
await step(A, 'limited-link', async () => {
  const link = await api(alice.token, 'POST', `/api/chats/${groupId}/invite-links`, { title: '一次', usage_limit: 1 })
  oneUse = link.token
})
await step(C, 'join-by-limited-link', async () => {
  await C.goto(`${BASE}/joinchat/${oneUse}`)
  if (process.env.CONSOLE) {
    await C.waitForTimeout(3000)
    console.log('  carol url', C.url(), (await C.content()).length, (await C.locator('body').innerText()).slice(0, 200))
  }
  await C.getByRole('button', { name: '加入群组' }).click()
  await C.locator('.tg-compose__input').waitFor({ timeout: 6000 })
})
const D = await page(dave)
await step(D, 'limited-link-full', async () => {
  await D.goto(`${BASE}/joinchat/${oneUse}`)
  await D.getByText(/名额已用完|名额已满/)
    .first()
    .waitFor({ timeout: 5000 })
})

// ---- admins
await step(A, 'promote-bob', async () => {
  await openAdmin(A, groupId)
  await A.locator('.tg-chatadmin__nav', { hasText: '成员' }).last().click()
  await A.locator('.tg-chatadmin__members').getByText(bob.username).first().click()
  await A.getByRole('button', { name: '设为管理员' }).click()
  await A.getByLabel('自定义头衔').fill('纪律委员')
  await A.locator('.tg-chatadmin__actions').getByRole('button', { name: '保存' }).click()
  // Lands on the administrators list, where the custom title is the badge.
  await A.locator('.tg-chatadmin__member', { hasText: bob.username }).getByText('纪律委员').waitFor({ timeout: 5000 })
})

// ---- restrict carol for one hour
await step(A, 'owner-badge-not-clipped', async () => {
  // The owner's row is static; its badge must sit fully inside the panel (TG-1203 fix).
  const row = A.locator('.tg-chatadmin__member', { hasText: alice.username })
  const badge = await row.locator('.tg-chatadmin__badge').boundingBox()
  const panel = await A.locator('.tg-chatadmin').boundingBox()
  if (!badge || badge.x + badge.width > panel.x + panel.width - 4)
    throw new Error(`owner badge clipped: ${JSON.stringify(badge)}`)
})
await step(A, 'restrict-carol', async () => {
  await openAdmin(A, groupId)
  await A.locator('.tg-chatadmin__nav', { hasText: '成员' }).last().click()
  await A.locator('.tg-chatadmin__members').getByText(carol.username).first().click()
  await A.getByRole('button', { name: '限制成员' }).click()
  await A.locator('.tg-chatadmin__editor').getByText('发送消息', { exact: true }).click()
  if (await A.getByRole('checkbox', { name: '发送消息' }).isChecked()) throw new Error('发送消息 still checked')
  await A.locator('.tg-chatadmin__editor').getByText('1 小时', { exact: true }).click()
  await A.getByRole('button', { name: '限制', exact: true }).click()
  await A.locator('.tg-chatadmin__members').getByText(carol.username).first().waitFor({ timeout: 5000 })
})
await step(C, 'restricted-composer', async () => {
  await C.goto(`${BASE}/chat/${groupId}`)
  await C.waitForTimeout(1500)
  const input = C.locator('.tg-compose__input')
  if ((await input.count()) && (await input.isEditable()))
    throw new Error('restricted member still has an editable composer')
  await C.getByText('管理员已限制你在此群组发送消息').waitFor({ timeout: 5000 })
})

// ---- the audit log: managers only; a member's info panel must not even ask (403 before)
await step(C, 'member-info-no-audit-request', async () => {
  let asked = false
  const watch = (r) => r.url().includes('/audit-events') && (asked = true)
  C.on('request', watch)
  await openInfo(C)
  await C.getByText('群组权限').first().waitFor({ timeout: 5000 })
  await C.waitForTimeout(1000)
  C.off('request', watch)
  if (asked) throw new Error('a plain member requested the audit log')
  if (await C.getByRole('button', { name: '操作日志' }).count()) throw new Error('member sees the audit log')
  await C.keyboard.press('Escape')
})
await step(A, 'owner-sees-audit-log', async () => {
  await A.goto(`${BASE}/chat/${groupId}`)
  await A.locator('.tg-compose__input').waitFor()
  await openInfo(A)
  await A.getByRole('button', { name: '操作日志' }).waitFor({ timeout: 5000 })
  await A.keyboard.press('Escape')
})

// ---- slow mode (also upgrades the group to a supergroup)
await api(alice.token, 'PUT', `/api/chats/${groupId}/members/${carol.user.id}/restrictions`, {
  denied_permissions: [],
  until: null,
})
await step(A, 'slow-mode-on', async () => {
  await openAdmin(A, groupId)
  await A.locator('.tg-chatadmin__nav', { hasText: '成员权限' }).click()
  await A.getByRole('radio', { name: '10 秒' }).click()
  await A.getByText('成员每 10 秒只能发送一条消息').first().waitFor({ timeout: 5000 })
  await A.getByRole('button', { name: '返回' }).click()
  await A.locator('.tg-chatadmin__type-label', { hasText: '超级群' }).waitFor({ timeout: 5000 })
})
await step(C, 'slow-mode-member-waits', async () => {
  await C.goto(`${BASE}/chat/${groupId}`)
  await say(C, '慢速第一条')
  await C.getByText('慢速模式已开启').first().waitFor({ timeout: 5000 })
})
await step(B, 'slow-mode-admin-exempt', async () => {
  await B.goto(`${BASE}/chat/${groupId}`)
  for (const text of ['管理员一', '管理员二']) await say(B, text)
  if (await B.getByText('慢速模式已开启').count()) throw new Error('administrator shown a slow-mode wait')
})
await api(alice.token, 'PUT', `/api/chats/${groupId}/slow-mode`, { seconds: 0 })

// ---- public username; an outsider finds and joins
const handle = `tg1203_${stamp}`
await step(A, 'public-username', async () => {
  await openAdmin(A, groupId)
  await A.getByLabel('用户名').fill(handle)
  await A.getByText(`@${handle} 可以使用`).waitFor({ timeout: 5000 })
  await A.locator('.tg-publiclink').getByRole('button', { name: '保存' }).click()
  await A.getByText(/任何人都可以通过/)
    .first()
    .waitFor({ timeout: 5000 })
})
await step(D, 'public-join', async () => {
  await D.goto(`${BASE}/public/${handle}`)
  await D.getByText(`@${handle}`).first().waitFor({ timeout: 5000 })
  await D.getByRole('button', { name: '加入群组' }).click()
  await D.locator('.tg-compose__input').waitFor({ timeout: 6000 })
})

// ---- ban dave from the panel; he can neither read nor rejoin; then the removed-users list
await step(A, 'ban-dave', async () => {
  await openAdmin(A, groupId)
  await A.locator('.tg-chatadmin__nav', { hasText: '成员' }).last().click()
  await A.locator('.tg-chatadmin__members').getByText(dave.username).first().click()
  await A.getByRole('button', { name: '封禁并移出群组' }).click()
  await A.getByRole('button', { name: '封禁', exact: true }).click()
  await A.locator('.tg-chatadmin__heading', { hasText: '成员' }).waitFor()
  await A.waitForTimeout(800)
  if (await A.locator('.tg-chatadmin__members').getByText(dave.username).count()) throw new Error('dave still listed')
})
await step(D, 'banned-cannot-rejoin', async () => {
  const read = await call(dave.token, 'GET', `/api/chats/${groupId}/messages`)
  if (read.status !== 403) throw new Error(`banned read → ${read.status}`)
  await D.goto(`${BASE}/public/${handle}`)
  await D.getByRole('button', { name: '加入群组' }).click()
  await D.getByText('无法加入，可能已被封禁。').waitFor({ timeout: 5000 })
})
await step(A, 'removed-users-unban', async () => {
  await A.getByRole('button', { name: '返回' }).click()
  await A.locator('.tg-chatadmin__nav', { hasText: '已封禁的用户' }).click()
  const row = A.locator('.tg-chatadmin__member', { hasText: dave.username })
  await row.getByRole('button', { name: '解除封禁' }).click()
  await A.getByText('没有被封禁的用户。').waitFor({ timeout: 5000 })
  const again = await call(dave.token, 'POST', `/api/public/${handle}/join`)
  if (again.status >= 300) throw new Error(`unbanned dave cannot rejoin: ${again.status}`)
})

// ---- channel: create with signatures, post, a subscriber, views, subscriber count
const channelTitle = `频道 ${stamp}`
let channelId = ''
await step(A, 'create-channel', async () => {
  await A.goto(`${BASE}/`)
  await A.getByRole('button', { name: '新建' }).first().click()
  await A.getByRole('menuitem', { name: '新建频道' }).click()
  await A.getByLabel('频道名称').fill(channelTitle)
  await A.getByRole('button', { name: '创建频道' }).click()
  await A.waitForURL(/\/chat\/[0-9a-f-]{36}/, { timeout: 6000 })
  channelId = A.url().match(/[0-9a-f-]{36}/)[0]
  await say(A, '第一篇帖子')
  if (await A.locator('.tg-message', { hasText: '第一篇帖子' }).locator('.tg-channel-post__author').count())
    throw new Error('signature shown while signatures are off')
})
await step(B, 'subscribe', async () => {
  const links = await api(alice.token, 'GET', `/api/chats/${channelId}/invite-links`)
  await B.goto(`${BASE}/joinchat/${links.links[0].token}`)
  await B.getByRole('button', { name: '加入频道' }).click()
  await B.locator('.tg-channel-footer').waitFor({ timeout: 6000 })
  if (await B.locator('.tg-compose__input').count()) throw new Error('subscriber has a composer')
  await B.locator('.tg-message', { hasText: '第一篇帖子' })
    .locator('.tg-channel-post__views')
    .first()
    .waitFor({ state: 'attached' })
})
await step(A, 'subscribers-and-views', async () => {
  await A.reload()
  await A.getByText('2 位订阅者').first().waitFor({ timeout: 6000 })
  await A.locator('.tg-message', { hasText: '第一篇帖子' })
    .locator('.tg-channel-post__views')
    .first()
    .waitFor({ state: 'attached' })
  const views = (
    await A.locator('.tg-message', { hasText: '第一篇帖子' }).locator('.tg-channel-post__views').allTextContents()
  ).join(' ')
  if (!/[12]/.test(views)) throw new Error(`views: ${views}`)
})
await step(A, 'signatures-toggle', async () => {
  await openAdmin(A, channelId)
  await A.getByRole('switch', { name: /消息署名/ }).click()
  await A.waitForTimeout(800)
  if ((await A.getByRole('switch', { name: /消息署名/ }).getAttribute('aria-checked')) !== 'true')
    throw new Error('signatures switch did not turn on')
  await A.keyboard.press('Escape')
  await A.goto(`${BASE}/chat/${channelId}`)
  await say(A, '署名帖子')
  await A.locator('.tg-message', { hasText: '署名帖子' })
    .locator('.tg-channel-post__author')
    .first()
    .waitFor({ state: 'attached', timeout: 5000 })
})

// ---- comments: link the group as the discussion, post, a subscriber comments
await step(A, 'link-discussion', async () => {
  await openAdmin(A, channelId)
  await A.getByLabel('选择讨论组').selectOption({ label: groupTitle })
  await A.locator('.tg-discussion-link').getByRole('button', { name: '关联' }).click()
  await A.locator('.tg-discussion-link').getByRole('button', { name: '取消关联' }).waitFor({ timeout: 5000 })
  await A.keyboard.press('Escape')
  await A.goto(`${BASE}/chat/${channelId}`)
  await say(A, '欢迎评论')
  await A.locator('.tg-message', { hasText: '欢迎评论' }).locator('.tg-comments-entry').waitFor({ timeout: 6000 })
})
await step(B, 'comment', async () => {
  await B.goto(`${BASE}/chat/${channelId}`)
  await B.locator('.tg-message', { hasText: '欢迎评论' }).locator('.tg-comments-entry').click()
  await B.getByLabel('写评论').fill('第一条评论')
  await B.locator('.tg-comments__sheet').getByRole('button', { name: '发送' }).click()
  await B.locator('.tg-comments__text', { hasText: '第一条评论' }).waitFor({ timeout: 5000 })
})

// ---- lifecycle: rename, a member leaves, the owner deletes
const renamed = `${groupTitle} 改`
await step(A, 'edit-profile', async () => {
  await A.goto(`${BASE}/chat/${groupId}`)
  await A.locator('.tg-compose__input').waitFor()
  await openInfo(A)
  await A.getByRole('button', { name: '编辑资料' }).click()
  await A.getByLabel('名称').fill(renamed)
  await A.getByRole('button', { name: '保存' }).last().click()
  await A.locator('header').getByText(renamed).first().waitFor({ timeout: 5000 })
})
await step(C, 'member-leaves', async () => {
  goneChat = groupId
  await C.goto(`${BASE}/chat/${groupId}`)
  await C.locator('.tg-compose__input').waitFor()
  await openInfo(C)
  await C.getByRole('button', { name: '退出群组' }).click()
  await C.getByRole('button', { name: '离开', exact: true }).click()
  await C.waitForTimeout(1200)
  if (await C.locator('.tg-chatlist').getByText(renamed).count()) throw new Error('left chat still listed')
  const read = await call(carol.token, 'GET', `/api/chats/${groupId}/messages`)
  if (read.status !== 403) throw new Error(`left member read → ${read.status}`)
})
await step(A, 'owner-deletes', async () => {
  await A.getByRole('button', { name: '删除群组' }).click()
  await A.getByRole('button', { name: '删除', exact: true }).click()
  await B.waitForTimeout(2500)
  if (await B.getByText(renamed).count()) throw new Error('deleted group still in a member’s list')
  const gone = await call(bob.token, 'GET', `/api/chats/${groupId}/messages`)
  if (gone.status < 400) throw new Error(`deleted group still readable: ${gone.status}`)
})

// ---- end
{
  console.log('errors', errors, 'bad', [...new Set(bad)])
  console.log(`${n - failed}/${n} steps ok`)
  if (failed || errors.length) process.exitCode = 1
  await browser.close()
}
