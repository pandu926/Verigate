import { useMutation, useQueryClient } from '@tanstack/react-query';
import { submitPresentation } from '@/lib/api';
import type { SubmitProofRequest, SubmitProofResponse } from '@/types/portal';

interface UseSubmitProofOptions {
  readonly caseId: string;
  readonly onSuccess?: (data: SubmitProofResponse) => void;
  readonly onError?: (error: Error) => void;
}

/**
 * TanStack Query mutation hook for submitting a verifiable presentation.
 * On success, invalidates requirements and completeness queries to refresh UI.
 */
export function useSubmitProof({ caseId, onSuccess, onError }: UseSubmitProofOptions) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (request: SubmitProofRequest) => submitPresentation(caseId, request),
    onSuccess: (data) => {
      queryClient.invalidateQueries({ queryKey: ['requirements', caseId] });
      queryClient.invalidateQueries({ queryKey: ['completeness', caseId] });
      onSuccess?.(data);
    },
    onError: (error: Error) => {
      onError?.(error);
    },
  });
}
