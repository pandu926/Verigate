import { NavLink, useLocation, useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { GridIcon, DocumentIcon, EyeIcon, ShieldIcon, ActivityIcon } from '@/components/ui/Icons';
import { useHealthCheck } from '@/hooks/useHealthCheck';
import { useAuthStore } from '@/stores/auth';
import '@/styles/app-shell.css';

interface AppShellProps {
  children: React.ReactNode;
}

export function AppShell({ children }: AppShellProps) {
  const location = useLocation();
  const navigate = useNavigate();
  const { data: health } = useHealthCheck();
  const { role, logout } = useAuthStore();
  const isAuthenticated = health?.agent_identity?.authenticated;

  const handleLogout = () => {
    logout();
    navigate('/login', { replace: true });
  };

  return (
    <div className="app-shell">
      <aside className="app-shell__sidebar">
        <div className="app-shell__logo">
          <ShieldIcon size={24} />
          <span>Verigate</span>
        </div>

        <nav className="app-shell__nav">
          {role === 'reviewer' && (
            <NavLink to="/dashboard" className={({ isActive }) => `app-shell__link ${isActive ? 'app-shell__link--active' : ''}`}>
              <GridIcon size={18} />
              <span>Dashboard</span>
            </NavLink>
          )}
          {role === 'counterparty' && (
            <NavLink to="/portal" className={({ isActive }) => `app-shell__link ${isActive ? 'app-shell__link--active' : ''}`}>
              <DocumentIcon size={18} />
              <span>My Cases</span>
            </NavLink>
          )}
          <NavLink to="/dashboard" className={() => `app-shell__link ${location.pathname.startsWith('/privacy/') ? 'app-shell__link--active' : ''}`}>
            <EyeIcon size={18} />
            <span>Privacy View</span>
          </NavLink>
          <NavLink to="/evidence" className={({ isActive }) => `app-shell__link ${isActive ? 'app-shell__link--active' : ''}`}>
            <ActivityIcon size={18} />
            <span>Evidence Chain</span>
          </NavLink>
        </nav>

        <div className="app-shell__footer">
          <div className="app-shell__status">
            <ActivityIcon size={14} />
            <span className={`app-shell__status-dot ${health?.database_connected ? 'app-shell__status-dot--connected' : ''}`} />
            <span className="app-shell__status-text">
              {health?.database_connected ? 'Connected' : 'Offline'}
            </span>
          </div>
          <div className="app-shell__agent">
            <span className="app-shell__agent-label">Agent</span>
            <span className={`app-shell__agent-badge ${isAuthenticated ? 'app-shell__agent-badge--auth' : 'app-shell__agent-badge--mock'}`}>
              {isAuthenticated ? 'T3 Live' : 'Dev Mode'}
            </span>
          </div>
          <div className="app-shell__user">
            <span className="app-shell__role-badge">{role}</span>
            <button className="app-shell__logout" onClick={handleLogout} data-testid="logout-btn">
              Logout
            </button>
          </div>
        </div>
      </aside>

      <main className="app-shell__main">
        <motion.div
          key={location.pathname}
          initial={{ opacity: 0, y: 8 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.3, ease: 'easeOut' }}
          className="app-shell__content"
        >
          {children}
        </motion.div>
      </main>
    </div>
  );
}
