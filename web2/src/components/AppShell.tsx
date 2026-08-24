import {
  CompassOutlined,
  ContactsOutlined,
  DashboardOutlined,
  MessageOutlined,
  SettingOutlined,
} from '@ant-design/icons'
import { Badge, Layout, Tooltip } from 'antd'
import { NavLink, Outlet } from 'react-router'
import { useAuth } from '../features/auth/auth-context'
import { UserAvatar } from './UserAvatar'

const links = [
  { to: '/chat', label: '消息', icon: <MessageOutlined /> },
  { to: '/contacts', label: '联系人', icon: <ContactsOutlined /> },
  { to: '/discover', label: '发现', icon: <CompassOutlined /> },
  { to: '/admin', label: '管理', icon: <DashboardOutlined /> },
]

function RailLink({ to, label, icon }: (typeof links)[number]) {
  return (
    <Tooltip title={label} placement="right">
      <NavLink
        to={to}
        className={({ isActive }) =>
          `flex h-11 w-11 items-center justify-center rounded-md text-lg transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-white ${
            isActive ? 'bg-[#ecf8f3] text-[#087f5b]' : 'text-[#aab8b3] hover:bg-white/10 hover:text-white'
          }`
        }
        aria-label={label}
      >
        {icon}
      </NavLink>
    </Tooltip>
  )
}

export function AppShell() {
  const { user } = useAuth()

  return (
    <Layout className="app-shell">
      <aside className="hidden w-[76px] shrink-0 flex-col items-center bg-[#15211d] py-4 md:flex">
        <NavLink
          to="/chat"
          className="mb-7 flex h-11 w-11 items-center justify-center rounded-md bg-[#f1b84b] text-xl font-black text-[#15211d]"
          aria-label="栖语首页"
        >
          栖
        </NavLink>
        <nav className="flex flex-1 flex-col items-center gap-3" aria-label="主导航">
          {links.map((link) => (
            <RailLink key={link.to} {...link} />
          ))}
        </nav>
        <div className="flex flex-col items-center gap-3">
          <Tooltip title="设置" placement="right">
            <NavLink
              to="/settings"
              className={({ isActive }) =>
                `flex h-11 w-11 items-center justify-center rounded-md text-lg ${
                  isActive ? 'bg-[#ecf8f3] text-[#087f5b]' : 'text-[#aab8b3] hover:bg-white/10 hover:text-white'
                }`
              }
              aria-label="设置"
            >
              <SettingOutlined />
            </NavLink>
          </Tooltip>
          <Badge dot color="#51cf66">
            <UserAvatar emoji={user?.avatar_emoji} name={user?.username ?? '?'} size={38} />
          </Badge>
        </div>
      </aside>

      <main className="workspace-surface min-w-0 flex-1">
        <Outlet />
      </main>

      <nav className="fixed inset-x-0 bottom-0 z-30 flex h-[60px] items-center justify-around border-t border-[#dfe5e2] bg-white md:hidden">
        {links.map((link) => (
          <NavLink
            key={link.to}
            to={link.to}
            className={({ isActive }) =>
              `flex h-full min-w-16 flex-col items-center justify-center gap-0.5 text-[11px] ${
                isActive ? 'text-[#087f5b]' : 'text-[#6b7772]'
              }`
            }
          >
            <span className="text-lg">{link.icon}</span>
            {link.label}
          </NavLink>
        ))}
        <NavLink
          to="/settings"
          className={({ isActive }) =>
            `flex h-full min-w-16 flex-col items-center justify-center gap-0.5 text-[11px] ${
              isActive ? 'text-[#087f5b]' : 'text-[#6b7772]'
            }`
          }
        >
          <SettingOutlined className="text-lg" />
          设置
        </NavLink>
      </nav>
    </Layout>
  )
}
