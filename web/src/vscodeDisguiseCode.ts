type CodeTone = 'blue' | 'cyan' | 'green' | 'orange' | 'plain' | 'purple' | 'yellow'

interface CodeToken {
  text: string
  tone?: CodeTone
}

export const VSCODE_SAMPLE_LINES: CodeToken[][] = [
  [
    { text: 'import', tone: 'purple' },
    { text: ' { createServer } ', tone: 'plain' },
    { text: 'from', tone: 'purple' },
    { text: " 'node:http'", tone: 'orange' },
  ],
  [
    { text: 'import', tone: 'purple' },
    { text: ' { router } ', tone: 'plain' },
    { text: 'from', tone: 'purple' },
    { text: " './router'", tone: 'orange' },
  ],
  [],
  [
    { text: 'const', tone: 'blue' },
    { text: ' port = Number(process.env.', tone: 'plain' },
    { text: 'PORT', tone: 'cyan' },
    { text: ' ?? ', tone: 'plain' },
    { text: "'3000'", tone: 'orange' },
    { text: ')', tone: 'plain' },
  ],
  [],
  [
    { text: 'const', tone: 'blue' },
    { text: ' server = ', tone: 'plain' },
    { text: 'createServer', tone: 'yellow' },
    { text: '(', tone: 'plain' },
    { text: 'async', tone: 'blue' },
    { text: ' (request, response) => {', tone: 'plain' },
  ],
  [
    { text: '  const', tone: 'blue' },
    { text: ' startedAt = performance.', tone: 'plain' },
    { text: 'now', tone: 'yellow' },
    { text: '()', tone: 'plain' },
  ],
  [
    { text: '  const', tone: 'blue' },
    { text: ' result = ', tone: 'plain' },
    { text: 'await', tone: 'purple' },
    { text: ' router.', tone: 'plain' },
    { text: 'resolve', tone: 'yellow' },
    { text: '(request)', tone: 'plain' },
  ],
  [],
  [
    { text: '  response.', tone: 'plain' },
    { text: 'writeHead', tone: 'yellow' },
    { text: '(result.status, {', tone: 'plain' },
  ],
  [
    { text: "    'content-type'", tone: 'orange' },
    { text: ': ', tone: 'plain' },
    { text: "'application/json'", tone: 'orange' },
    { text: ',', tone: 'plain' },
  ],
  [
    { text: "    'server-timing'", tone: 'orange' },
    { text: ': ', tone: 'plain' },
    { text: '`app;dur=${', tone: 'green' },
    { text: '(performance.', tone: 'plain' },
    { text: 'now', tone: 'yellow' },
    { text: '() - startedAt).', tone: 'plain' },
    { text: 'toFixed', tone: 'yellow' },
    { text: '(1)', tone: 'plain' },
    { text: '}`', tone: 'green' },
  ],
  [{ text: '  })', tone: 'plain' }],
  [
    { text: '  response.', tone: 'plain' },
    { text: 'end', tone: 'yellow' },
    { text: '(JSON.', tone: 'plain' },
    { text: 'stringify', tone: 'yellow' },
    { text: '(result.body))', tone: 'plain' },
  ],
  [{ text: '})', tone: 'plain' }],
  [],
  [
    { text: 'server.', tone: 'plain' },
    { text: 'listen', tone: 'yellow' },
    { text: '(port, ', tone: 'plain' },
    { text: '() =>', tone: 'blue' },
    { text: ' {', tone: 'plain' },
  ],
  [
    { text: '  console.', tone: 'plain' },
    { text: 'log', tone: 'yellow' },
    { text: '(', tone: 'plain' },
    { text: '`ready on http://localhost:${port}`', tone: 'green' },
    { text: ')', tone: 'plain' },
  ],
  [{ text: '})', tone: 'plain' }],
]
