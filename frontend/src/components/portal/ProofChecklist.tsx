import { type CSSProperties, useState, useCallback } from 'react';
import type { ProofRequirement, RequirementCompleteness } from '@/types/portal';
import { SubmissionCard } from './SubmissionCard';
import './ProofChecklist.css';

interface ProofChecklistProps {
  /** List of proof requirements for the case. */
  readonly requirements: readonly ProofRequirement[];
  /** Per-requirement completeness data for status enrichment. */
  readonly byRequirement: readonly RequirementCompleteness[];
  /** Case ID for submission requests. */
  readonly caseId: string;
  /** Called when a proof is successfully submitted. */
  readonly onSubmitSuccess?: () => void;
}

type StepVariant = 'verified' | 'current' | 'pending' | 'failed';

function resolveStepVariant(
  requirement: ProofRequirement,
  isFirstPending: boolean,
): StepVariant {
  if (requirement.status === 'verified') return 'verified';
  if (requirement.status === 'failed') return 'failed';
  if (requirement.status === 'submitted') return 'current';
  if (isFirstPending) return 'current';
  return 'pending';
}

/**
 * Vertical journey-style checklist of proof requirements.
 * Renders a timeline with connecting line, animated nodes,
 * and expandable submission cards for each requirement step.
 */
export function ProofChecklist({ requirements, byRequirement, caseId, onSubmitSuccess }: ProofChecklistProps) {
  const [expandedId, setExpandedId] = useState<string | null>(null);
  let firstPendingFound = false;

  const handleToggle = useCallback((reqId: string) => {
    setExpandedId((prev) => (prev === reqId ? null : reqId));
  }, []);

  const handleSubmitSuccess = useCallback(() => {
    setExpandedId(null);
    onSubmitSuccess?.();
  }, [onSubmitSuccess]);

  return (
    <div className="proof-checklist" role="list" aria-label="Proof requirements checklist">
      {requirements.map((req, index) => {
        const isFirstPending =
          !firstPendingFound &&
          (req.status === 'pending' || req.status === 'submitted');

        if (isFirstPending) {
          firstPendingFound = true;
        }

        const variant = resolveStepVariant(req, isFirstPending);
        const completeness = byRequirement.find((r) => r.requirement_id === req.id);

        const stepDelay: CSSProperties = {
          animationDelay: `${index * 80}ms`,
        };

        const showSubmitAction = variant === 'current';

        return (
          <div
            key={req.id}
            className="proof-checklist__step"
            style={stepDelay}
            role="listitem"
          >
            <StepNode variant={variant} />
            <div className="proof-checklist__content">
              <StepCard
                requirement={req}
                variant={variant}
                completeness={completeness}
                onSubmitClick={() => handleToggle(req.id)}
                showAction={showSubmitAction}
              />
              {showSubmitAction && (
                <SubmissionCard
                  requirement={req}
                  caseId={caseId}
                  isExpanded={expandedId === req.id}
                  onToggle={() => handleToggle(req.id)}
                  onSubmitSuccess={handleSubmitSuccess}
                />
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* --- Sub-components --- */

function StepNode({ variant }: { readonly variant: StepVariant }) {
  return (
    <div className={`proof-checklist__node proof-checklist__node--${variant}`}>
      {variant === 'verified' && (
        <svg className="proof-checklist__checkmark" viewBox="0 0 16 16" aria-hidden="true">
          <polyline points="3 8 7 12 13 4" />
        </svg>
      )}
      {variant === 'failed' && (
        <svg width="12" height="12" viewBox="0 0 16 16" aria-hidden="true">
          <line x1="4" y1="4" x2="12" y2="12" stroke="var(--color-bg-primary)" strokeWidth="2.5" strokeLinecap="round" />
          <line x1="12" y1="4" x2="4" y2="12" stroke="var(--color-bg-primary)" strokeWidth="2.5" strokeLinecap="round" />
        </svg>
      )}
    </div>
  );
}

interface StepCardProps {
  readonly requirement: ProofRequirement;
  readonly variant: StepVariant;
  readonly completeness: RequirementCompleteness | undefined;
  readonly onSubmitClick: () => void;
  readonly showAction: boolean;
}

function StepCard({ requirement, variant, completeness, onSubmitClick, showAction }: StepCardProps) {
  const cardClass = `proof-checklist__card proof-checklist__card--${variant}`;
  const badgeClass = `proof-checklist__badge proof-checklist__badge--${requirement.status}`;

  const claimsVerified = completeness?.verified_claims_count ?? 0;
  const claimsRequired = completeness?.required_claims_count ?? requirement.required_claims.length;

  return (
    <div className={cardClass}>
      <div className="proof-checklist__header">
        <span className="proof-checklist__type">{requirement.claim_type.replace(/_/g, ' ')}</span>
        <span className={badgeClass}>{requirement.status}</span>
      </div>

      <p className="proof-checklist__description">{requirement.description}</p>

      {requirement.required_claims.length > 0 && (
        <div className="proof-checklist__claims">
          {requirement.required_claims.map((claim) => (
            <span key={claim} className="proof-checklist__claim-tag">
              {claim}
            </span>
          ))}
        </div>
      )}

      {completeness && (
        <div style={{ marginTop: 'var(--space-sm)' }}>
          <span
            style={{
              fontSize: 'var(--text-xs)',
              fontFamily: 'var(--font-mono)',
              color: 'var(--color-text-tertiary)',
            }}
          >
            {claimsVerified}/{claimsRequired} claims verified
          </span>
        </div>
      )}

      {showAction && (
        <button className="proof-checklist__action" type="button" onClick={onSubmitClick}>
          Submit Proof
          <svg width="14" height="14" viewBox="0 0 16 16" fill="none" aria-hidden="true">
            <path d="M3 8h10M9 4l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
        </button>
      )}
    </div>
  );
}
