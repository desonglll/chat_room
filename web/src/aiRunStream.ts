export async function readSseJsonStream<T>(response: Response, onMessage: (value: T) => void): Promise<void> {
  if (!response.body) throw new Error('AI 流响应为空')
  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  let buffer = ''

  for (;;) {
    const { done, value } = await reader.read()
    buffer += decoder.decode(value, { stream: !done }).replaceAll('\r\n', '\n')
    let boundary = buffer.indexOf('\n\n')
    while (boundary !== -1) {
      const block = buffer.slice(0, boundary)
      buffer = buffer.slice(boundary + 2)
      const data = block
        .split('\n')
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).trimStart())
        .join('\n')
      if (data) onMessage(JSON.parse(data) as T)
      boundary = buffer.indexOf('\n\n')
    }
    if (done) return
  }
}
