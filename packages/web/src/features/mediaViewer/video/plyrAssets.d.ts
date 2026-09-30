/** Vite `?raw` imports: the file's text as the default export. */
declare module '*.svg?raw' {
  const content: string
  export default content
}
