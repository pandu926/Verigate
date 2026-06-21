import { useQuery } from '@tanstack/react-query';
import { fetchRequirements } from '@/lib/api';
import type { RequirementsResponse } from '@/types/portal';

/**
 * TanStack Query hook for fetching case proof requirements.
 * Refetches on window focus to keep the checklist current.
 */
export function useRequirements(caseId: string) {
  return useQuery<RequirementsResponse, Error>({
    queryKey: ['requirements', caseId],
    queryFn: () => fetchRequirements(caseId),
    enabled: caseId.length > 0,
    refetchOnWindowFocus: true,
    retry: 2,
    staleTime: 30_000,
  });
}
