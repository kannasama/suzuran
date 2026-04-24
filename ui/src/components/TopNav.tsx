import { useEffect, useRef, useState } from 'react'
import { NavLink, useNavigate } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from '../contexts/AuthContext'
import { getStagedCount } from '../api/ingest'
import { issuesApi } from '../api/issues'

export function TopNav() {
  const { user, logout } = useAuth()
  const navigate = useNavigate()
  const [menuOpen, setMenuOpen] = useState(false)
  const menuRef = useRef<HTMLDivElement>(null)

  const { data: inboxCount = 0 } = useQuery({
    queryKey: ['ingest-count'],
    queryFn: getStagedCount,
    refetchInterval: 30_000,
  })

  const { data: issuesCount = 0 } = useQuery({
    queryKey: ['issues-count'],
    queryFn: () => issuesApi.count(),
    refetchInterval: 60_000,
  })

  // Close on outside click
  useEffect(() => {
    if (!menuOpen) return
    const handler = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) {
        setMenuOpen(false)
      }
    }
    document.addEventListener('mousedown', handler)
    return () => document.removeEventListener('mousedown', handler)
  }, [menuOpen])

  const handleSignOut = async () => {
    setMenuOpen(false)
    await logout()
    navigate('/login')
  }

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
    <header className="flex items-center gap-0 bg-bg-base border-b border-border px-4 flex-shrink-0 h-10">
      <span className="text-accent text-sm font-semibold mr-4 tracking-tight">
        suzuran
      </span>
      {navItem('/', 'Library')}
      <NavLink
        to="/ingest"
        className={({ isActive }) =>
          `text-xs px-3 py-2 border-b-2 transition-colors inline-flex items-center ${
            isActive
              ? 'text-accent border-accent'
              : 'text-text-muted border-transparent hover:text-text-secondary'
          }`
        }
      >
        Ingest
        {inboxCount > 0 && (
          <span className="ml-1.5 inline-flex items-center justify-center
                           h-4 min-w-[1rem] px-1 rounded-full
                           text-[10px] font-bold
                           bg-accent text-bg-base">
            {inboxCount > 99 ? '99+' : inboxCount}
          </span>
        )}
      </NavLink>
      <NavLink
        to="/issues"
        className={({ isActive }) =>
          `text-xs px-3 py-2 border-b-2 transition-colors inline-flex items-center ${
            isActive
              ? 'text-accent border-accent'
              : 'text-text-muted border-transparent hover:text-text-secondary'
          }`
        }
      >
        Issues
        {issuesCount > 0 && (
          <span className="ml-1.5 inline-flex items-center justify-center
                           h-4 min-w-[1rem] px-1 rounded-full
                           text-[10px] font-bold
                           bg-yellow-500 text-bg-base">
            {issuesCount > 99 ? '99+' : issuesCount}
          </span>
        )}
      </NavLink>
      {navItem('/jobs', 'Jobs')}
      {(user?.role === 'admin' || user?.role === 'library_admin') && navItem('/organization', 'Organization')}

      <div className="ml-auto flex items-center">
        {user && (
          <div ref={menuRef} className="relative">
            <button
              onClick={() => setMenuOpen(v => !v)}
              className="flex items-center gap-1 px-2 py-1 text-xs text-text-muted hover:text-text-secondary transition-colors"
            >
              {user.username}
              <svg
                className={`w-3 h-3 transition-transform ${menuOpen ? 'rotate-180' : ''}`}
                viewBox="0 0 12 12"
                fill="none"
                stroke="currentColor"
                strokeWidth="1.5"
              >
                <path d="M2 4l4 4 4-4" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            </button>

            {menuOpen && (
              <div className="absolute right-0 top-full mt-1 w-44 bg-bg-surface border border-border rounded shadow-lg z-50 py-1">
                <div className="px-3 py-1.5 border-b border-border">
                  <p className="text-xs font-medium text-text-primary truncate">{user.username}</p>
                  <p className="text-xs text-text-muted capitalize">{user.role}</p>
                </div>
                <NavLink
                  to="/account"
                  onClick={() => setMenuOpen(false)}
                  className="flex items-center px-3 py-1.5 text-xs text-text-secondary hover:bg-bg-hover hover:text-text-primary transition-colors"
                >
                  Account
                </NavLink>
                <NavLink
                  to="/settings"
                  onClick={() => setMenuOpen(false)}
                  className="flex items-center px-3 py-1.5 text-xs text-text-secondary hover:bg-bg-hover hover:text-text-primary transition-colors"
                >
                  Settings
                </NavLink>
                <div className="border-t border-border mt-1 pt-1">
                  <button
                    onClick={handleSignOut}
                    className="w-full text-left flex items-center px-3 py-1.5 text-xs text-destructive hover:bg-bg-hover transition-colors"
                  >
                    Sign out
                  </button>
                </div>
              </div>
            )}
          </div>
        )}
      </div>
    </header>
  )
}
