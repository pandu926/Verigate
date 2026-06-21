import { useQuery } from '@tanstack/react-query';
import { fetchHealth } from '@/lib/api';
import type { HealthResponse } from '@/types/domain';

const HEALTH_POLL_INTERVAL_MS = 10_000;

/**
 * TanStack Query hook for polling the backend health endpoint.
 * Polls every 10 seconds to keep status indicators live.
 */
export function useHealthCheck() {
  return useQuery<HealthResponse, Error>({
    queryKey: ['health'],
    queryFn: fetchHealth,
    refetchInterval: HEALTH_POLL_INTERVAL_MS,
    retry: 1,
    staleTime: HEALTH_POLL_INTERVAL_MS / 2,
  });
}
