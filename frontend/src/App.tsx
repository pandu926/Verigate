import { BrowserRouter, Routes, Route, Navigate } from 'react-router-dom';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { AppShell } from '@/components/layout/AppShell';
import { Portal } from '@/pages/Portal';
import { ReviewerDashboard } from '@/pages/ReviewerDashboard';
import { PrivacySplitScreen } from '@/pages/PrivacySplitScreen';
import { EvidenceChain } from '@/pages/EvidenceChain';
import { Landing } from '@/pages/Landing';
import { Login } from '@/pages/Login';
import { ProtectedRoute } from '@/components/auth/ProtectedRoute';
import '@/styles/global.css';

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      refetchOnWindowFocus: false,
      retry: 1,
    },
  },
});

export function App() {
  return (
    <QueryClientProvider client={queryClient}>
      <BrowserRouter>
        <Routes>
          <Route path="/" element={<Landing />} />
          <Route path="/login" element={<Login />} />
          <Route
            path="/dashboard"
            element={
              <ProtectedRoute allowedRoles={['reviewer']}>
                <AppShell>
                  <ReviewerDashboard />
                </AppShell>
              </ProtectedRoute>
            }
          />
          <Route
            path="/portal/:caseId"
            element={
              <ProtectedRoute>
                <AppShell>
                  <Portal />
                </AppShell>
              </ProtectedRoute>
            }
          />
          <Route
            path="/portal"
            element={
              <ProtectedRoute allowedRoles={['counterparty']}>
                <AppShell>
                  <ReviewerDashboard />
                </AppShell>
              </ProtectedRoute>
            }
          />
          <Route
            path="/privacy/:caseId"
            element={
              <ProtectedRoute>
                <AppShell>
                  <PrivacySplitScreen />
                </AppShell>
              </ProtectedRoute>
            }
          />
          <Route
            path="/evidence"
            element={
              <AppShell>
                <EvidenceChain />
              </AppShell>
            }
          />
          <Route path="*" element={<Navigate to="/login" replace />} />
        </Routes>
      </BrowserRouter>
    </QueryClientProvider>
  );
}
