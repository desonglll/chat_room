/**
 * TG-110 steps for `m1-chat.e2e.mjs` (kept apart so neither file crosses the size limit):
 * the quiz editor's label-less radio, a live poll vote, no edit on a poll, the info-panel
 * toggle, and the settings panel on desktop and on a phone. Runs in the chat the main script
 * set up, with A and B both inside it; leaves B back in that chat at 1280×800.
 */
/** The earlier steps leave A scrolled up (selection): jump back to the newest message. */
async function toLatest(page) {
  const jump = page.getByRole('button', { name: '回到最新消息' })
  for (let attempt = 0; attempt < 3 && (await jump.isVisible()); attempt += 1) {
    await jump.click()
    await page.waitForTimeout(600)
  }
}

export async function runBatch2Steps({ a, b, check, shot, BASE_URL, ALICE, chatId }) {
  await check('quiz editor: a click on a label-less radio circle selects it (Radio fix)', async () => {
    await a.getByRole('button', { name: '添加附件' }).click()
    await a.getByRole('menuitem', { name: '投票' }).click()
    const dialog = a.getByRole('dialog', { name: '新建投票' })
    await dialog.waitFor()
    await dialog.getByText('测验模式').click()
    const dot = a.getByRole('dialog').locator('.tg-poll-create__option .tg-radio__dot').nth(1)
    const box = await dot.boundingBox()
    await a.mouse.click(box.x + box.width / 2, box.y + box.height / 2)
    const radio = a.getByRole('dialog').locator('.tg-poll-create__option input[type="radio"]').nth(1)
    if (!(await radio.isChecked())) throw new Error('clicking the ring did not check the radio')
    await shot(a, 'a-quiz-radio')
    await a.getByRole('dialog').getByText('测验模式').click()
  })

  await check('A creates a poll from the attach menu; A votes, B votes, A sees the live percentage', async () => {
    const dialog = a.getByRole('dialog', { name: '新建投票' })
    await dialog.getByLabel('问题').fill('午饭吃什么？')
    await dialog.getByLabel('选项 1').fill('面')
    await dialog.getByLabel('选项 2').fill('饭')
    await dialog.getByRole('button', { name: '创建', exact: true }).click()
    await dialog.waitFor({ state: 'detached' })
    const pollA = a.locator('.tg-poll', { hasText: '午饭吃什么？' }).last()
    const pollB = b.locator('.tg-poll', { hasText: '午饭吃什么？' }).last()
    await toLatest(a)
    await toLatest(b)
    await pollA.waitFor()
    await pollB.waitFor({ timeout: 10_000 })
    await pollA.getByRole('button', { name: '面' }).click()
    await pollA.locator('.tg-poll__percent', { hasText: '100%' }).waitFor()
    await pollB.getByRole('button', { name: '饭' }).click()
    // A never reloads: 50% can only come from B's vote via the live poll_updated frame.
    await pollA.locator('.tg-poll__percent', { hasText: '50%' }).first().waitFor({ timeout: 10_000 })
    if ((await pollA.locator('.tg-poll__percent', { hasText: '50%' }).count()) !== 2)
      throw new Error('A does not show 50% / 50% after B voted')
    await shot(a, 'a-poll-live')
  })

  await check('a poll message offers no edit action', async () => {
    await a.locator('.tg-message', { hasText: '午饭吃什么？' }).last().locator('.tg-message__column').first().click({
      button: 'right',
    })
    await a.getByRole('menuitem', { name: '删除' }).waitFor()
    if ((await a.getByRole('menuitem', { name: '编辑' }).count()) !== 0) throw new Error('poll offers 编辑')
    await a.keyboard.press('Escape')
  })

  await check('the header toggles the info panel: open, then closed with the exit slide', async () => {
    const header = a.getByRole('button', { name: '查看会话信息' })
    await header.click()
    await a.locator('.tg-chatinfo').waitFor()
    if ((await header.getAttribute('aria-expanded')) !== 'true') throw new Error('aria-expanded not true')
    await a.waitForTimeout(500)
    await shot(a, 'a-info-open')
    await header.click()
    await a.locator('.tg-chatinfo[data-closing]').waitFor({ timeout: 2_000 })
    await a.locator('.tg-chatinfo').waitFor({ state: 'detached', timeout: 3_000 })
    if ((await header.getAttribute('aria-expanded')) !== 'false') throw new Error('aria-expanded not false')
  })

  await check(
    'settings: hamburger → 设置 → 隐私与安全 → 隐私 renders; 设备 lists this device; Esc closes',
    async () => {
      await a.getByRole('button', { name: '主菜单' }).first().click()
      await a.getByRole('menuitem', { name: '设置' }).click()
      const panel = a.getByRole('region', { name: '设置' })
      await panel.getByText(`@${ALICE.username}`).waitFor()
      await a.waitForTimeout(500)
      await shot(a, 'a-settings-root')
      await panel.getByRole('button', { name: '隐私与安全' }).click()
      await panel.getByRole('button', { name: '两步验证' }).waitFor()
      await panel.getByRole('button', { name: '隐私', exact: true }).click()
      await panel.locator('.tg-privacy__row').first().waitFor()
      await a.waitForTimeout(500)
      await shot(a, 'a-settings-privacy')
      await panel.getByRole('button', { name: '返回' }).first().click()
      await panel.getByRole('button', { name: '两步验证' }).click()
      await panel.locator('.tg-security').waitFor()
      await panel.getByRole('button', { name: '返回' }).first().click()
      await panel.getByRole('button', { name: '返回' }).first().click()
      await panel.getByRole('button', { name: '数据与存储' }).click()
      await panel.getByText('即将推出').waitFor()
      await panel.getByRole('button', { name: '返回' }).first().click()
      await panel.getByRole('button', { name: '设备' }).click()
      await panel.getByText('当前设备').waitFor()
      await a.waitForTimeout(500)
      await shot(a, 'a-settings-devices')
      await a.keyboard.press('Escape')
      await a.locator('.tg-settings').waitFor({ state: 'detached', timeout: 3_000 })
    },
  )

  await check('settings on a phone: full screen, 我的账号 saves the display name', async () => {
    await b.setViewportSize({ width: 390, height: 780 })
    await b.goto(`${BASE_URL}/`)
    await b.getByRole('button', { name: '主菜单' }).first().click()
    await b.getByRole('menuitem', { name: '设置' }).click()
    const panel = b.getByRole('region', { name: '设置' })
    await panel.getByRole('button', { name: '我的账号' }).click()
    await panel.getByLabel('名称').fill('Bob 本人')
    await panel.getByRole('button', { name: '保存' }).click()
    await panel.getByText('Bob 本人').waitFor()
    const width = await panel.evaluate((node) => node.getBoundingClientRect().width)
    if (Math.abs(width - 390) > 1) throw new Error(`settings is ${width}px wide on a 390px screen`)
    await b.waitForTimeout(500)
    await shot(b, 'b-settings-mobile')
    await panel.getByRole('button', { name: '关闭设置' }).click()
    await b.locator('.tg-settings').waitFor({ state: 'detached', timeout: 3_000 })
    await b.setViewportSize({ width: 1280, height: 800 })
    await b.goto(`${BASE_URL}/chat/${chatId()}`)
  })
}
