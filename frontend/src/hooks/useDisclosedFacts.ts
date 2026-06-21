import { useQuery } from '@tanstack/react-query';
import { fetchDisclosedFacts } from '@/lib/api';
import type { DisclosedFact } from '@/types/demo';

/**
 * TanStack Query hook for fetching disclosed facts for a case.
 * These are the privacy-filtered claims that the AI agent sees.
 */
export function useDisclosedFacts(caseId: string) {
  return useQuery<DisclosedFact[], Error>({
    queryKey: ['disclosed-facts', caseId],
    queryFn: () => fetchDisclosedFacts(caseId),
    enabled: caseId.length > 0,
    retry: 1,
    staleTime: 30_000,
  });
}
