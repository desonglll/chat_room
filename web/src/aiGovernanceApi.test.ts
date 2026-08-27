import { afterEach, describe, expect, mock, test } from 'bun:test'
import { getAiUsage, saveAiGovernance, type AiGovernanceSettings } from './aiGovernanceApi'

const originalFetch = globalThis.fetch

afterEach(() => {
  globalThis.fetch = originalFetch
})

describe('AI governance API', () => {
  test('sends limits, allowlist, and privacy-safe price fields', async () => {
    const fetchMock = mock(async (_path: string, options?: RequestInit) =>
      Response.json(JSON.parse(String(options?.body))),
    )
    globalThis.fetch = fetchMock as typeof fetch
    const settings = {
      max_concurrent_runs: 4,
      daily_user_token_limit: 10_000,
      daily_room_token_limit: null,
      allowlist_enabled: true,
      updated_at: '2026-08-27T00:00:00Z',
      models: [
        {
          id: 'model-1',
          label: 'Production',
          provider: 'openai',
          model: 'gpt-test',
          ready: true,
          allowed: true,
          input_price_micros_per_million: 2_000_000,
          output_price_micros_per_million: 8_000_000,
        },
      ],
    } satisfies AiGovernanceSettings

    await saveAiGovernance('admin-token', settings)
    const payload = JSON.parse(String(fetchMock.mock.calls[0]![1]?.body))

    expect(payload.models[0]).toEqual({
      id: 'model-1',
      allowed: true,
      input_price_micros_per_million: 2_000_000,
      output_price_micros_per_million: 8_000_000,
    })
    expect(JSON.stringify(payload)).not.toContain('prompt')
    expect(JSON.stringify(payload)).not.toContain('evidence')
  })

  test('requests only the selected aggregate dimension', async () => {
    const fetchMock = mock(async () =>
      Response.json({ group_by: 'model', from: '', to: '', token_source: 'estimated', items: [] }),
    )
    globalThis.fetch = fetchMock as typeof fetch

    await getAiUsage('admin-token', 'model')

    expect(String(fetchMock.mock.calls[0]![0])).toBe('/api/admin/ai-usage?group_by=model')
  })
})
