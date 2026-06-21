import { motion } from 'framer-motion';

interface TimelineStepProps {
  step: string;
  success: boolean;
  result: Record<string, unknown>;
  elapsed_ms: number;
  index: number;
}

function getStepMeta(step: string) {
  if (step === 'commit-plan') return { icon: '📋', label: 'Commit Plan', color: 'var(--color-accent)' };
  if (step.startsWith('verify-credential')) return { icon: '✓', label: 'Verify Credential', color: 'var(--color-success)' };
  if (step === 'assess-risk') return { icon: '🧠', label: 'AI Risk Assessment', color: 'var(--color-warning)' };
  if (step === 'decide') return { icon: '⚖️', label: 'Decision', color: 'var(--color-accent)' };
  return { icon: '•', label: step, color: 'var(--color-text-secondary)' };
}

export function TimelineStep({ step, success, result, elapsed_ms, index }: TimelineStepProps) {
  const meta = getStepMeta(step);
  const r = result || {};

  return (
    <motion.div
      className="timeline-step"
      style={{ '--step-color': meta.color } as React.CSSProperties}
      initial={{ opacity: 0, x: -20 }}
      animate={{ opacity: 1, x: 0 }}
      transition={{ delay: index * 0.3, duration: 0.4, ease: [0.16, 1, 0.3, 1] }}
    >
      <div className="timeline-step__connector" />
      <div className="timeline-step__dot" />
      <div className="timeline-step__card">
        <div className="timeline-step__header">
          <span className="timeline-step__icon">{meta.icon}</span>
          <span className="timeline-step__label">{meta.label}</span>
          <span className="timeline-step__status">
            {success ? '✓' : '✗'}
          </span>
          <span className="timeline-step__time">{elapsed_ms}ms</span>
        </div>
        <div className="timeline-step__body">
          {step === 'commit-plan' && (
            <>
              <Row label="Steps" value={`${r.steps_count || (r.steps as string[])?.length || '?'} steps committed`} />
              <Row label="Committed By" value={String(r.committed_by || '')} mono />
              {String(r.committed_by || '').includes('bec292d') && (
                <div className="timeline-step__badge timeline-step__badge--delegation">
                  Delegation Enforced — counterparty DID
                </div>
              )}
            </>
          )}
          {step.startsWith('verify-credential') && (
            <>
              <Row label="Verified" value={r.verified ? 'Yes (Ed25519)' : 'Failed'} />
              <Row label="Facts Extracted" value={String(r.facts_count || 0)} />
              <Row label="Algorithms" value={(r.algorithms_used as string[])?.join(', ') || 'EdDSA'} />
              {r.result_hash && <Row label="Hash" value={String(r.result_hash).slice(0, 16) + '...'} mono />}
            </>
          )}
          {step === 'assess-risk' && (
            <>
              <Row label="AI Decision" value={String(r.decision || 'N/A')} />
              <Row label="Confidence" value={String(r.confidence || 'N/A')} />
              <div className="timeline-step__reasoning">
                {String(r.reasoning || '')}
              </div>
              <div className="timeline-step__badge timeline-step__badge--tee">
                Executed inside T3N TEE enclave
              </div>
            </>
          )}
          {step === 'decide' && (
            <>
              <Row label="Decision" value={String(r.decision || 'N/A')} />
              <Row label="Confidence" value={String(r.confidence || 'N/A')} />
              <Row label="Steps Completed" value={String(r.steps_completed || 'N/A')} />
            </>
          )}
        </div>
      </div>
    </motion.div>
  );
}

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="timeline-step__row">
      <span className="timeline-step__row-label">{label}</span>
      <span className={`timeline-step__row-value${mono ? ' timeline-step__row-value--mono' : ''}`}>
        {value}
      </span>
    </div>
  );
}
