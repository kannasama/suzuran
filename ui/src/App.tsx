import { Navigate, Route, Routes } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { useAuth } from './contexts/AuthContext'
import { getSetupStatus } from './api/auth'
import { LoginPage } from './pages/LoginPage'
import { RegisterPage } from './pages/RegisterPage'
import { LibraryPage } from './pages/LibraryPage'
import OrganizationPage from './pages/OrganizationPage'
import IngestPage from './pages/IngestPage'
import SettingsPage from './pages/SettingsPage'
import AccountPage from './pages/AccountPage'
import TwoFactorPage from './pages/TwoFactorPage'
import JobsPage from './pages/JobsPage'

function useSetupStatus() {
  return useQuery({
    queryKey: ['setup-status'],
    queryFn: getSetupStatus,
    staleTime: Infinity,   // only fetched once per session
    retry: false,
  })
}

function AppRoutes() {
  const { data: setup, isLoading: setupLoading } = useSetupStatus()
  const { user, loading: authLoading } = useAuth()

  // Wait for both setup status and auth check before rendering anything
  if (setupLoading || authLoading) return null

  // First-run: no users exist — force setup regardless of route
  if (setup?.needs_setup) {
    return (
      <Routes>
        <Route path="/register" element={<RegisterPage setupMode />} />
        <Route path="*" element={<Navigate to="/register" replace />} />
      </Routes>
    )
  }

  // Normal routing: setup complete
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/login/2fa" element={<TwoFactorPage />} />
      {/* Register is first-run only — redirect away once setup is done */}
      <Route path="/register" element={<Navigate to="/login" replace />} />
      <Route
        path="/organization"
        element={user ? <OrganizationPage /> : <Navigate to="/login" replace />}
      />
      <Route
        path="/ingest"
        element={user ? <IngestPage /> : <Navigate to="/login" replace />}
      />
      {/* Legacy inbox route — redirect to /ingest */}
      <Route path="/inbox" element={<Navigate to="/ingest" replace />} />
      <Route
        path="/settings"
        element={user ? <SettingsPage /> : <Navigate to="/login" replace />}
      />
      <Route
        path="/account"
        element={user ? <AccountPage /> : <Navigate to="/login" replace />}
      />
      <Route
        path="/jobs"
        element={user ? <JobsPage /> : <Navigate to="/login" replace />}
      />
      <Route
        path="/*"
        element={user ? <LibraryPage /> : <Navigate to="/login" replace />}
      />
    </Routes>
  )
}

export function App() {
  return <AppRoutes />
}
