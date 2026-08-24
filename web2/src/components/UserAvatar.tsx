import { Avatar } from 'antd'

interface UserAvatarProps {
  emoji?: string
  name: string
  size?: number | 'large' | 'small' | 'default'
  className?: string
}

const COLORS = ['#087f5b', '#2563eb', '#a13d63', '#7c3aed', '#b45309', '#0e7490']

export function UserAvatar({ emoji, name, size = 'default', className }: UserAvatarProps) {
  const color = COLORS[[...name].reduce((sum, character) => sum + character.charCodeAt(0), 0) % COLORS.length]
  return (
    <Avatar className={className} size={size} style={{ backgroundColor: color, flexShrink: 0 }}>
      {emoji || name.trim().slice(0, 1).toUpperCase() || '?'}
    </Avatar>
  )
}
