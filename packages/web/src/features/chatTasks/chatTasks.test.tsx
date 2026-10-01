import { describe, expect, test } from 'bun:test'
import { renderToStaticMarkup } from 'react-dom/server'
import type { TasksApi } from '@tg/core'
import { AuditLogEntry } from './AuditLogEntry'
import { TasksSection } from './TasksSection'

describe('TG-706 tasks and audit log', () => {
  test('the tasks section offers adding a task', () => {
    const html = renderToStaticMarkup(<TasksSection chatId="c1" api={{} as TasksApi} />)
    expect(html).toContain('任务')
    expect(html).toContain('新任务')
  })

  test('the audit entry stays hidden until the server lets the viewer read the log', () => {
    expect(renderToStaticMarkup(<AuditLogEntry chatId="c1" api={{} as TasksApi} />)).toBe('')
  })
})
