import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useAuthStore } from '@/stores/auth';

interface CreateCaseRequest {
  entity_name: string;
  entity_type: string;
  workflow_type: string;
  jurisdiction: string;
  relationship_goal: string;
}

const API_BASE = import.meta.env.VITE_API_URL || '';

export function useCreateCase({ onSuccess, onError }: { onSuccess?: () => void; onError?: (msg: string) => void } = {}) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (data: CreateCaseRequest) => {
      const token = useAuthStore.getState().getToken();
      const resp = await fetch(`${API_BASE}/api/cases`, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json',
          ...(token ? { 'Authorization': `Bearer ${token}` } : {}),
        },
        body: JSON.stringify(data),
      });
      if (!resp.ok) {
        const text = await resp.text();
        throw new Error(text || `HTTP ${resp.status}`);
      }
      return resp.json();
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['cases'] });
      onSuccess?.();
    },
    onError: (err: Error) => {
      onError?.(err.message);
    },
  });
}
