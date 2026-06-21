import { useQuery } from '@tanstack/react-query';
import { fetchCases } from '@/lib/api';
import type { DemoCase } from '@/types/demo';

export function useCases() {
  return useQuery<DemoCase[], Error>({
    queryKey: ['cases'],
    queryFn: () => fetchCases(),
    retry: 1,
    staleTime: 30_000,
  });
}
