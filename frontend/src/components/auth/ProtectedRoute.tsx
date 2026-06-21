import { Navigate } from 'react-router-dom';
import { useAuthStore } from '@/stores/auth';

interface ProtectedRouteProps {
  children: React.ReactNode;
  allowedRoles?: Array<'reviewer' | 'counterparty'>;
}

export function ProtectedRoute({ children, allowedRoles }: ProtectedRouteProps) {
  const { isAuthenticated, role } = useAuthStore();

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  if (allowedRoles && role && !allowedRoles.includes(role)) {
    const home = role === 'reviewer' ? '/dashboard' : '/portal';
    return <Navigate to={home} replace />;
  }

  return <>{children}</>;
}
