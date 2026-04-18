import { NavLink } from 'react-router-dom'
import { useAuth } from '../contexts/AuthContext'

export function TopNav() {
  const { user, logout } = useAuth()

  const navItem = (to: string, label: string) => (
    <NavLink
      to={to}
      className={({ isActive }) =>
        `text-xs px-3 py-2 border-b-2 transition-colors ${
          isActive
            ? 'text-accent border-accent'
            : 'text-text-muted border-transparent hover:text-text-secondary'
        }`
      }
    >
      {label}
    </NavLink>
  )

  return (
    <header className="flex items-center gap-0 bg-bg-base border-b border-border px-4 flex-shrink-0 h-9">
      <span className="text-accent text-sm font-semibold mr-4 tracking-tight">
        suzuran
      </span>
      {navItem('/', 'Library')}
      {navItem('/inbox', 'Inbox')}
      {navItem('/issues', 'Issues')}
      {navItem('/jobs', 'Jobs')}
      <div className="ml-auto flex items-center gap-3">
        <NavLink
          to="/settings"
          className="text-xs text-text-muted hover:text-text-secondary"
        >
          Settings
        </NavLink>
        {user && (
          <button
            onClick={logout}
            className="text-xs text-text-muted hover:text-text-secondary"
          >
            {user.username}
          </button>
        )}
      </div>
    </header>
  )
}
