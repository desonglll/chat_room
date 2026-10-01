import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import { MemoryRouter } from 'react-router-dom'
import { PasswordSettingsPage } from './PasswordSettingsPage'
import { newPasswordProblem } from './passwordRules'

describe('TG-704 password and account', () => {
  test('the new password follows the server rule and must be repeated', () => {
    expect(newPasswordProblem('', 'abcdefgh', 'abcdefgh')).toBe('w.password.currentRequired')
    expect(newPasswordProblem('old-pass', 'short', 'short')).toBe('w.password.tooShort')
    expect(newPasswordProblem('old-pass', 'x'.repeat(257), 'x'.repeat(257))).toBe('w.password.tooLong')
    expect(newPasswordProblem('old-pass', 'new-pass-1', 'new-pass-2')).toBe('w.password.mismatch')
    expect(newPasswordProblem('same-pass', 'same-pass', 'same-pass')).toBe('w.password.same')
    expect(newPasswordProblem('old-pass', 'new-pass-1', 'new-pass-1')).toBe('')
  })

  test('the page offers the change and a guarded account deletion', () => {
    const html = renderToStaticMarkup(
      <MemoryRouter>
        <PasswordSettingsPage />
      </MemoryRouter>,
    )
    expect(html).toContain('修改登录密码')
    expect(html).toContain('current-password')
    expect(html).toContain('删除账号')
    expect(html).not.toContain('永久删除账号')
  })
})
