import { Navigate, Route, Routes } from 'react-router-dom'
import { useAuth } from './contexts/AuthContext'
import { LoginPage } from './pages/LoginPage'
import { RegisterPage } from './pages/RegisterPage'
import { LibraryPage } from './pages/LibraryPage'

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth()
  if (loading) return null
  if (!user) return <Navigate to="/login" replace />
  return <>{children}</>
}

export function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/register" element={<RegisterPage />} />
      <Route
        path="/*"
        element={
          <ProtectedRoute>
            <LibraryPage />
          </ProtectedRoute>
        }
      />
    </Routes>
  )
}
