import { useQuery } from '@tanstack/react-query';
import { fetchCompleteness } from '@/lib/api';
import type { CompletenessResponse } from '@/types/portal';

const COMPLETENESS_POLL_INTERVAL_MS = 5_000;

/**
 * TanStack Query hook for fetching case completeness data.
 * Polls every 5 seconds to keep the progress ring live.
 */
export function useCompleteness(caseId: string) {
  return useQuery<CompletenessResponse, Error>({
    queryKey: ['completeness', caseId],
    queryFn: () => fetchCompleteness(caseId),
    enabled: caseId.length > 0,
    refetchInterval: COMPLETENESS_POLL_INTERVAL_MS,
    retry: 2,
    staleTime: COMPLETENESS_POLL_INTERVAL_MS / 2,
  });
}
