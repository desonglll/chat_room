try {
  const theme = localStorage.getItem('chat-room.theme')
  const dark = theme === 'dark' || (theme !== 'light' && matchMedia('(prefers-color-scheme: dark)').matches)
  if (dark) document.documentElement.setAttribute('data-theme', 'dark')
  document.documentElement.style.colorScheme = dark ? 'dark' : 'light'
  document.querySelector('meta[name="theme-color"]')?.setAttribute('content', dark ? '#111614' : '#f7f9f8')
} catch {}
