import { NavLink } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '../contexts/AuthContext'
import { tagSuggestionsApi } from '../api/tagSuggestions'

export function TopNav() {
  const { user, logout } = useAuth()

  const { data: inboxCount = 0 } = useQuery({
    queryKey: ['inbox-count'],
    queryFn: () => tagSuggestionsApi.count(),
    refetchInterval: 30_000,
  })

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
      <NavLink
        to="/inbox"
        className={({ isActive }) =>
          `text-xs px-3 py-2 border-b-2 transition-colors inline-flex items-center ${
            isActive
              ? 'text-accent border-accent'
              : 'text-text-muted border-transparent hover:text-text-secondary'
          }`
        }
      >
        Inbox
        {inboxCount > 0 && (
          <span className="ml-1.5 inline-flex items-center justify-center
                           h-4 min-w-[1rem] px-1 rounded-full
                           text-[10px] font-bold
                           bg-accent text-bg-base">
            {inboxCount > 99 ? '99+' : inboxCount}
          </span>
        )}
      </NavLink>
      {navItem('/issues', 'Issues')}
      {navItem('/jobs', 'Jobs')}
      {user?.role === 'admin' && navItem('/organization', 'Organization')}
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
