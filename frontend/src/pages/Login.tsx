import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion } from 'framer-motion';
import { useAuthStore } from '@/stores/auth';
import './Login.css';

export function Login() {
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState('');
  const [loading, setLoading] = useState(false);
  const navigate = useNavigate();
  const login = useAuthStore(s => s.login);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!email.trim() || !password.trim()) {
      setError('Email and password are required');
      return;
    }
    setError('');
    setLoading(true);

    try {
      await login(email, password);
      const role = useAuthStore.getState().role;
      navigate(role === 'reviewer' ? '/dashboard' : '/portal', { replace: true });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Login failed');
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-page">
      <motion.div
        className="login-card"
        initial={{ opacity: 0, y: 24 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
      >
        <div className="login-header">
          <div className="login-logo">
            <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
              <rect width="32" height="32" rx="8" fill="#14b8a6" fillOpacity="0.15" />
              <path d="M8 16l6 6 10-12" stroke="#14b8a6" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </div>
          <h1 className="login-title">Verigate</h1>
          <p className="login-subtitle">Autonomous Counterparty Agent</p>
        </div>

        <form className="login-form" onSubmit={handleSubmit}>
          <label className="login-field">
            <span className="login-label">Email</span>
            <input
              type="email"
              name="email"
              value={email}
              onChange={e => setEmail(e.target.value)}
              placeholder="reviewer@verigate.io"
              autoComplete="email"
              autoFocus
              aria-label="Email address"
            />
          </label>

          <label className="login-field">
            <span className="login-label">Password</span>
            <input
              type="password"
              name="password"
              value={password}
              onChange={e => setPassword(e.target.value)}
              placeholder="••••••••"
              autoComplete="current-password"
              aria-label="Password"
            />
          </label>

          {error && (
            <motion.div
              className="login-error"
              initial={{ opacity: 0, height: 0 }}
              animate={{ opacity: 1, height: 'auto' }}
            >
              {error}
            </motion.div>
          )}

          <button
            type="submit"
            className="login-btn"
            disabled={loading}
          >
            {loading ? 'Signing in...' : 'Sign In'}
          </button>
        </form>

        <div className="login-footer">
          <p className="login-hint">Demo accounts:</p>
          <div className="login-accounts">
            <button
              type="button"
              className="login-demo-btn"
              onClick={() => { setEmail('reviewer@verigate.io'); setPassword('reviewer123'); }}
            >
              Reviewer
            </button>
            <button
              type="button"
              className="login-demo-btn"
              onClick={() => { setEmail('counterparty@verigate.io'); setPassword('counterparty123'); }}
            >
              Counterparty
            </button>
          </div>
        </div>
      </motion.div>
    </div>
  );
}
