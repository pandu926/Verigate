import { useState, useCallback, useRef, useEffect } from 'react';
import type { ProofRequirement } from '@/types/portal';
import { useSubmitProof } from '@/hooks/useSubmitProof';
import './SubmissionCard.css';

interface SubmissionCardProps {
  readonly requirement: ProofRequirement;
  readonly caseId: string;
  readonly isExpanded: boolean;
  readonly onToggle: () => void;
  readonly onSubmitSuccess: () => void;
}

interface VPValidation {
  readonly isValid: boolean;
  readonly error: string | null;
  readonly disclosedFields: readonly string[];
  readonly totalFields: number;
}

const DEBOUNCE_MS = 300;

/** Validate VP JSON and extract disclosed field names. */
function validateVPJson(raw: string): VPValidation {
  if (!raw.trim()) {
    return { isValid: false, error: null, disclosedFields: [], totalFields: 0 };
  }

  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch {
    return { isValid: false, error: 'Invalid JSON syntax', disclosedFields: [], totalFields: 0 };
  }

  if (typeof parsed !== 'object' || parsed === null) {
    return { isValid: false, error: 'VP must be a JSON object', disclosedFields: [], totalFields: 0 };
  }

  const vp = parsed as Record<string, unknown>;

  // Check for VP structure markers
  if (!vp['@context'] && !vp['type'] && !vp['verifiableCredential']) {
    return {
      isValid: false,
      error: 'Missing VP structure (expected @context, type, or verifiableCredential)',
      disclosedFields: [],
      totalFields: 0,
    };
  }

  // Extract disclosed field names from verifiableCredential array
  const credentials = vp['verifiableCredential'];
  const disclosedFields: string[] = [];
  let totalFields = 0;

  if (Array.isArray(credentials)) {
    for (const cred of credentials) {
      if (typeof cred === 'object' && cred !== null) {
        const credObj = cred as Record<string, unknown>;
        const subject = credObj['credentialSubject'] as Record<string, unknown> | undefined;
        if (subject && typeof subject === 'object') {
          const keys = Object.keys(subject).filter((k) => k !== 'id');
          disclosedFields.push(...keys);
          totalFields += keys.length;
        }
      } else if (typeof cred === 'string') {
        // JWT-encoded credential — count it as opaque (we can't peek inside SD-JWT without decoding)
        totalFields += 1;
        disclosedFields.push('[JWT credential]');
      }
    }
  }

  return { isValid: true, error: null, disclosedFields, totalFields };
}

/**
 * Expandable proof submission card with VP JSON validation and disclosure preview.
 * Styled as a secure vault — dark surface with glowing accent border when expanded.
 */
export function SubmissionCard({
  requirement,
  caseId,
  isExpanded,
  onToggle,
  onSubmitSuccess,
}: SubmissionCardProps) {
  const [vpJson, setVpJson] = useState('');
  const [validation, setValidation] = useState<VPValidation>({
    isValid: false,
    error: null,
    disclosedFields: [],
    totalFields: 0,
  });
  const [showSuccess, setShowSuccess] = useState(false);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const { mutate: submitProof, isPending, isError, error: submitError } = useSubmitProof({
    caseId,
    onSuccess: () => {
      setShowSuccess(true);
      setTimeout(() => {
        setShowSuccess(false);
        setVpJson('');
        setValidation({ isValid: false, error: null, disclosedFields: [], totalFields: 0 });
        onSubmitSuccess();
      }, 1500);
    },
  });

  // Focus textarea when card expands
  useEffect(() => {
    if (isExpanded && textareaRef.current) {
      textareaRef.current.focus();
    }
  }, [isExpanded]);

  const handleInputChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value;
    setVpJson(value);

    // Debounced validation
    if (debounceRef.current) {
      clearTimeout(debounceRef.current);
    }
    debounceRef.current = setTimeout(() => {
      setValidation(validateVPJson(value));
    }, DEBOUNCE_MS);
  }, []);

  const handleSubmit = useCallback(() => {
    if (!validation.isValid || isPending) return;

    let parsedVp: object;
    try {
      parsedVp = JSON.parse(vpJson) as object;
    } catch {
      return;
    }

    submitProof({
      requirement_id: requirement.id,
      credential_type: requirement.claim_type,
      requirement_claim_type: requirement.claim_type,
      raw_vp: parsedVp,
    });
  }, [validation.isValid, isPending, vpJson, submitProof, requirement.id, requirement.claim_type]);

  const cardClass = [
    'submission-card',
    isExpanded ? 'submission-card--expanded' : '',
    showSuccess ? 'submission-card--success' : '',
  ].filter(Boolean).join(' ');

  return (
    <div className={cardClass}>
      {/* Collapsed header */}
      <button
        className="submission-card__header"
        onClick={onToggle}
        type="button"
        aria-expanded={isExpanded}
        aria-controls={`submission-form-${requirement.id}`}
      >
        <div className="submission-card__header-left">
          <svg className="submission-card__vault-icon" width="18" height="18" viewBox="0 0 18 18" fill="none" aria-hidden="true">
            <rect x="2" y="5" width="14" height="11" rx="2" stroke="currentColor" strokeWidth="1.5" />
            <path d="M6 5V4a3 3 0 0 1 6 0v1" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            <circle cx="9" cy="10.5" r="1.5" stroke="currentColor" strokeWidth="1.5" />
            <line x1="9" y1="12" x2="9" y2="13.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
          <span className="submission-card__header-title">Submit Proof</span>
        </div>
        <svg
          className={`submission-card__chevron ${isExpanded ? 'submission-card__chevron--open' : ''}`}
          width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true"
        >
          <path d="M3 5.5L7 9.5L11 5.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </button>

      {/* Expanded form */}
      <div
        id={`submission-form-${requirement.id}`}
        className="submission-card__body"
        aria-hidden={!isExpanded}
      >
        <div className="submission-card__form">
          {/* Textarea for VP JSON */}
          <label className="submission-card__label" htmlFor={`vp-input-${requirement.id}`}>
            Verifiable Presentation JSON
          </label>
          <textarea
            ref={textareaRef}
            id={`vp-input-${requirement.id}`}
            className={`submission-card__textarea ${
              validation.error ? 'submission-card__textarea--error' : ''
            } ${validation.isValid ? 'submission-card__textarea--valid' : ''}`}
            value={vpJson}
            onChange={handleInputChange}
            placeholder='{"@context": [...], "type": "VerifiablePresentation", "verifiableCredential": [...]}'
            rows={8}
            spellCheck={false}
            aria-describedby={validation.error ? `vp-error-${requirement.id}` : undefined}
          />

          {/* Validation error message */}
          {validation.error && (
            <p id={`vp-error-${requirement.id}`} className="submission-card__error" role="alert">
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none" aria-hidden="true">
                <circle cx="7" cy="7" r="6" stroke="currentColor" strokeWidth="1.2" />
                <line x1="7" y1="4" x2="7" y2="8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
                <circle cx="7" cy="10" r="0.6" fill="currentColor" />
              </svg>
              {validation.error}
            </p>
          )}

          {/* Disclosure preview */}
          {validation.isValid && validation.disclosedFields.length > 0 && (
            <div className="submission-card__disclosure">
              <h4 className="submission-card__disclosure-title">Fields to be disclosed</h4>
              <div className="submission-card__chips">
                {validation.disclosedFields.map((field, i) => (
                  <span key={`${field}-${i}`} className="submission-card__chip submission-card__chip--disclosed">
                    <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                      <circle cx="5" cy="5" r="4" stroke="currentColor" strokeWidth="1" />
                      <circle cx="5" cy="5" r="2" fill="currentColor" />
                    </svg>
                    {field}
                  </span>
                ))}
              </div>

              {/* Private fields hint */}
              {requirement.required_claims.length > 0 && (
                <>
                  <h4 className="submission-card__disclosure-title submission-card__disclosure-title--private">
                    Fields remaining private
                  </h4>
                  <div className="submission-card__chips">
                    {requirement.required_claims
                      .filter((c) => !validation.disclosedFields.includes(c))
                      .map((field) => (
                        <span key={field} className="submission-card__chip submission-card__chip--private">
                          <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
                            <rect x="2" y="4" width="6" height="4" rx="1" stroke="currentColor" strokeWidth="0.8" />
                            <path d="M3.5 4V3a1.5 1.5 0 0 1 3 0v1" stroke="currentColor" strokeWidth="0.8" />
                          </svg>
                          {field}
                        </span>
                      ))}
                  </div>
                </>
              )}
            </div>
          )}

          {/* API error */}
          {isError && (
            <p className="submission-card__error" role="alert">
              {submitError?.message ?? 'Submission failed. Please try again.'}
            </p>
          )}

          {/* Submit button */}
          <button
            className={`submission-card__submit ${showSuccess ? 'submission-card__submit--success' : ''}`}
            onClick={handleSubmit}
            disabled={!validation.isValid || isPending}
            type="button"
          >
            {isPending && (
              <svg className="submission-card__spinner" width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <circle cx="8" cy="8" r="6" stroke="currentColor" strokeWidth="2" strokeDasharray="28" strokeLinecap="round" />
              </svg>
            )}
            {showSuccess && (
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none" aria-hidden="true">
                <polyline points="3 8 7 12 13 4" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
              </svg>
            )}
            {!isPending && !showSuccess && 'Submit Presentation'}
            {isPending && 'Verifying...'}
            {showSuccess && 'Verified'}
          </button>
        </div>
      </div>
    </div>
  );
}
