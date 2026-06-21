import { create } from 'zustand';

const TOKEN_KEY = 'verigate_token';
const API_BASE = import.meta.env.VITE_API_URL || '';

interface AuthState {
  token: string | null;
  role: 'reviewer' | 'counterparty' | null;
  userId: string | null;
  isAuthenticated: boolean;
  login: (email: string, password: string) => Promise<void>;
  logout: () => void;
  getToken: () => string | null;
}

function decodePayload(token: string): { sub: string; role: string; exp: number } | null {
  try {
    const parts = token.split('.');
    if (parts.length !== 3) return null;
    const segment = parts[1] as string;
    const payload = JSON.parse(atob(segment));
    return payload;
  } catch {
    return null;
  }
}

function isTokenExpired(token: string): boolean {
  const payload = decodePayload(token);
  if (!payload) return true;
  return payload.exp * 1000 < Date.now();
}

function hydrateFromStorage(): Pick<AuthState, 'token' | 'role' | 'userId' | 'isAuthenticated'> {
  const token = localStorage.getItem(TOKEN_KEY);
  if (!token || isTokenExpired(token)) {
    localStorage.removeItem(TOKEN_KEY);
    return { token: null, role: null, userId: null, isAuthenticated: false };
  }
  const payload = decodePayload(token);
  if (!payload) {
    return { token: null, role: null, userId: null, isAuthenticated: false };
  }
  return {
    token,
    role: payload.role as 'reviewer' | 'counterparty',
    userId: payload.sub,
    isAuthenticated: true,
  };
}

export const useAuthStore = create<AuthState>((set, get) => ({
  ...hydrateFromStorage(),

  login: async (email: string, password: string) => {
    const res = await fetch(`${API_BASE}/api/auth/login`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ email, password }),
    });

    if (!res.ok) {
      const data = await res.json().catch(() => ({}));
      throw new Error(data.error || 'Login failed');
    }

    const data = await res.json();
    localStorage.setItem(TOKEN_KEY, data.token);
    set({
      token: data.token,
      role: data.role,
      userId: data.user_id,
      isAuthenticated: true,
    });
  },

  logout: () => {
    localStorage.removeItem(TOKEN_KEY);
    set({ token: null, role: null, userId: null, isAuthenticated: false });
  },

  getToken: () => {
    const { token } = get();
    if (!token || isTokenExpired(token)) {
      get().logout();
      return null;
    }
    return token;
  },
}));
