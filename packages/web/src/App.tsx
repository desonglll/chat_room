import { RouterProvider } from 'react-router-dom'
import { appRouter } from './app/router'

/** The TG-012 shell: routing skeleton over the login page and the three-pane workspace. */
export function App() {
  return <RouterProvider router={appRouter} />
}
