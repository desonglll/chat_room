import { App as AntApp, ConfigProvider } from 'antd'
import zhCN from 'antd/locale/zh_CN'
import { lazy, Suspense } from 'react'
import type { ReactNode } from 'react'
import { Navigate, Route, Routes } from 'react-router'
import { AppShell } from './components/AppShell'
import { RequireAuth } from './features/auth/RequireAuth'

const AdminPage = lazy(() => import('./pages/AdminPage').then((module) => ({ default: module.AdminPage })))
const AuthPage = lazy(() => import('./pages/AuthPage').then((module) => ({ default: module.AuthPage })))
const ChatPage = lazy(() => import('./pages/ChatPage').then((module) => ({ default: module.ChatPage })))
const ContactsPage = lazy(() => import('./pages/ContactsPage').then((module) => ({ default: module.ContactsPage })))
const DiscoverPage = lazy(() => import('./pages/DiscoverPage').then((module) => ({ default: module.DiscoverPage })))
const SettingsPage = lazy(() => import('./pages/SettingsPage').then((module) => ({ default: module.SettingsPage })))

function Page({ children }: { children: ReactNode }) {
  return <Suspense fallback={<div className="flex h-full items-center justify-center text-sm text-[#6f7c76]">加载中…</div>}>{children}</Suspense>
}

export default function App() {
  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        token: {
          colorPrimary: '#087f5b',
          colorInfo: '#2563eb',
          colorSuccess: '#16803c',
          colorWarning: '#c2410c',
          colorError: '#c83b45',
          borderRadius: 6,
          fontFamily:
            "Inter, 'SF Pro Text', 'PingFang SC', 'Microsoft YaHei', sans-serif",
        },
        components: {
          Button: { controlHeight: 38, controlHeightLG: 44 },
          Input: { controlHeight: 40 },
          Layout: { bodyBg: '#f4f6f8', siderBg: '#15211d' },
        },
      }}
    >
      <AntApp>
        <Routes>
          <Route path="/auth" element={<Page><AuthPage /></Page>} />
          <Route element={<RequireAuth />}>
            <Route element={<AppShell />}>
              <Route index element={<Navigate to="/chat" replace />} />
              <Route path="chat" element={<Page><ChatPage /></Page>} />
              <Route path="chat/:roomId" element={<Page><ChatPage /></Page>} />
              <Route path="discover" element={<Page><DiscoverPage /></Page>} />
              <Route path="contacts" element={<Page><ContactsPage /></Page>} />
              <Route path="settings" element={<Page><SettingsPage /></Page>} />
              <Route path="admin" element={<Page><AdminPage /></Page>} />
            </Route>
          </Route>
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </AntApp>
    </ConfigProvider>
  )
}
