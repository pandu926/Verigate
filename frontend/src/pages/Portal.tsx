import { useState, useCallback } from 'react';
import { useParams, Link } from 'react-router-dom';
import { useQueryClient } from '@tanstack/react-query';
import { motion, AnimatePresence } from 'framer-motion';
import { useRequirements } from '@/hooks/useRequirements';
import { useCompleteness } from '@/hooks/useCompleteness';
import { useSubmitProof } from '@/hooks/useSubmitProof';
import { useEventStream } from '@/hooks/useEventStream';
import { generateTestVp, triggerAssessment, fetchAssessment } from '@/lib/api';
import { ToastContainer, addToast } from '@/components/ui/Toast';
import { ShieldIcon, CheckIcon, ClockIcon, AlertIcon, SendIcon, ChevronRightIcon, RefreshIcon } from '@/components/ui/Icons';
import { AnimatedCounter } from '@/components/ui/AnimatedCounter';
import type { ProofRequirement, SSEEvent } from '@/types/portal';
import './Portal.css';

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

type TabId = 'requirements' | 'submissions' | 'assessment';

const CLAIM_TYPE_TO_VP_TYPE: Record<string, string> = {
  entity_registration: 'entity',
  authorized_signer: 'signer',
  jurisdiction_compliance: 'region',
  wallet_proof: 'wallet',
  beneficial_ownership: 'entity',
};

interface AssessmentResult {
  decision: string;
  confidence: number;
  summary_text: string;
  created_at: string;
}

export function Portal() {
  const { caseId } = useParams<{ caseId: string }>();
  const queryClient = useQueryClient();
  const [activeTab, setActiveTab] = useState<TabId>('requirements');
  const [submittingId, setSubmittingId] = useState<string | null>(null);
  const [submittedIds, setSubmittedIds] = useState<Set<string>>(new Set());
  const [assessmentLoading, setAssessmentLoading] = useState(false);
  const [assessment, setAssessment] = useState<AssessmentResult | null>(null);

  const validId = caseId && UUID_PATTERN.test(caseId);
  const { data: reqData, isLoading: reqLoading } = useRequirements(validId ? caseId : '');
  const { data: completeness } = useCompleteness(validId ? caseId : '');
  const { mutate: submitProof, isPending: isSubmitting } = useSubmitProof({
    caseId: caseId || '',
    onSuccess: () => {
      addToast('success', 'Credential verified in T3N TEE');
      setSubmittingId(null);
    },
    onError: () => {
      addToast('error', 'Submission failed — check credential format');
      setSubmittingId(null);
    },
  });

  const handleEvent = useCallback((event: SSEEvent) => {
    if (event.type === 'submission_verified') {
      addToast('success', 'Credential verified via TEE');
      queryClient.invalidateQueries({ queryKey: ['requirements', caseId] });
      queryClient.invalidateQueries({ queryKey: ['completeness', caseId] });
    } else if (event.type === 'assessment_complete') {
      addToast('success', 'AI assessment complete');
      loadAssessment();
    } else if (event.type === 'status_changed') {
      queryClient.invalidateQueries({ queryKey: ['requirements', caseId] });
    }
  }, [caseId, queryClient]);

  useEventStream({ caseId: caseId || '', onEvent: handleEvent, enabled: !!validId });

  const loadAssessment = async () => {
    if (!caseId) return;
    try {
      const result = await fetchAssessment(caseId) as AssessmentResult;
      if (result) setAssessment(result);
    } catch { /* no assessment yet */ }
  };

  if (!validId) {
    return (
      <div className="portal">
        <div className="portal__error">
          <AlertIcon size={24} />
          <h2>Invalid Case ID</h2>
          <p>Please select a case from the dashboard.</p>
          <Link to="/dashboard" className="portal__back-btn">Go to Dashboard</Link>
        </div>
      </div>
    );
  }

  const requirements = reqData?.data || [] as readonly ProofRequirement[];
  const verified = requirements.filter(r => r.status === 'verified').length;
  const total = requirements.length;
  const percentage = completeness?.percentage || (total > 0 ? Math.round((verified / total) * 100) : 0);

  const handleSubmit = async (req: ProofRequirement) => {
    setSubmittingId(req.id);
    setSubmittedIds(prev => new Set([...prev, req.id]));
    const vpType = CLAIM_TYPE_TO_VP_TYPE[req.claim_type] || 'entity';

    try {
      const vp = await generateTestVp(vpType);
      submitProof({
        requirement_id: req.id,
        credential_type: req.claim_type,
        requirement_claim_type: req.claim_type,
        raw_vp: vp,
      });
    } catch {
      addToast('error', 'Failed to generate credential');
      setSubmittingId(null);
      setSubmittedIds(prev => { const n = new Set(prev); n.delete(req.id); return n; });
    }
  };

  const handleTriggerAssessment = async () => {
    if (!caseId) return;
    setAssessmentLoading(true);
    try {
      await triggerAssessment(caseId);
      addToast('info', 'AI assessment triggered — 4-agent pipeline running in TEE...');
      setActiveTab('assessment');
      // Poll for result
      setTimeout(async () => {
        for (let i = 0; i < 12; i++) {
          await new Promise(r => setTimeout(r, 15000));
          try {
            const result = await fetchAssessment(caseId) as AssessmentResult;
            if (result && result.decision) {
              setAssessment(result);
              setAssessmentLoading(false);
              return;
            }
          } catch { /* still processing */ }
        }
        setAssessmentLoading(false);
        addToast('info', 'Assessment still processing — check back shortly');
      }, 5000);
    } catch {
      addToast('error', 'Failed to trigger assessment');
      setAssessmentLoading(false);
    }
  };

  return (
    <div className="portal">
      <ToastContainer />

      {/* Header */}
      <header className="portal__header">
        <div className="portal__header-left">
          <Link to="/dashboard" className="portal__breadcrumb">Dashboard</Link>
          <ChevronRightIcon size={14} />
          <span className="portal__breadcrumb-current">Case Portal</span>
        </div>
        <div className="portal__progress-badge">
          <AnimatedCounter value={percentage} />% Complete
        </div>
      </header>

      {/* Progress Bar */}
      <div className="portal__progress-bar">
        <motion.div
          className="portal__progress-fill"
          initial={{ width: 0 }}
          animate={{ width: `${percentage}%` }}
          transition={{ duration: 0.8, ease: 'easeOut' }}
        />
      </div>

      {/* Tabs */}
      <div className="portal__tabs">
        {(['requirements', 'submissions', 'assessment'] as TabId[]).map(tab => (
          <button
            key={tab}
            className={`portal__tab ${activeTab === tab ? 'portal__tab--active' : ''}`}
            onClick={() => setActiveTab(tab)}
          >
            {tab === 'requirements' && <ShieldIcon size={16} />}
            {tab === 'submissions' && <CheckIcon size={16} />}
            {tab === 'assessment' && <RefreshIcon size={16} />}
            <span>{tab.charAt(0).toUpperCase() + tab.slice(1)}</span>
            {tab === 'requirements' && <span className="portal__tab-count">{total}</span>}
            {tab === 'submissions' && <span className="portal__tab-count">{verified}</span>}
          </button>
        ))}
      </div>

      {/* Tab Content */}
      <AnimatePresence mode="wait">
        {activeTab === 'requirements' && (
          <motion.div
            key="requirements"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -12 }}
            transition={{ duration: 0.2 }}
            className="portal__panel"
          >
            {reqLoading ? (
              <div className="portal__panel-loading">Loading requirements...</div>
            ) : requirements.length === 0 ? (
              <div className="portal__panel-empty">No requirements defined yet.</div>
            ) : (
              <div className="portal__req-list">
                {requirements.map((req, i) => (
                  <motion.div
                    key={req.id}
                    className={`portal__req ${req.status === 'verified' ? 'portal__req--verified' : ''}`}
                    initial={{ opacity: 0, x: -16 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: i * 0.05 }}
                  >
                    <div className="portal__req-icon">
                      {req.status === 'verified' ? <CheckIcon size={16} /> : <ClockIcon size={16} />}
                    </div>
                    <div className="portal__req-info">
                      <h4 className="portal__req-type">{req.claim_type.replace(/_/g, ' ')}</h4>
                      <p className="portal__req-desc">{req.description || `Provide ${req.category} verification`}</p>
                      {req.required_claims && req.required_claims.length > 0 && (
                        <div className="portal__req-claims">
                          {req.required_claims.map(c => (
                            <span key={c} className="portal__req-claim">{c.replace(/_/g, ' ')}</span>
                          ))}
                        </div>
                      )}
                    </div>
                    <div className="portal__req-action">
                      {req.status === 'verified' || submittedIds.has(req.id) ? (
                        <span className="portal__req-done">
                          {req.status === 'verified' ? 'Verified in TEE' : 'Submitting...'}
                        </span>
                      ) : (
                        <button
                          className="portal__submit-btn"
                          onClick={() => handleSubmit(req)}
                          disabled={(isSubmitting && submittingId === req.id)}
                        >
                          {isSubmitting && submittingId === req.id ? (
                            <motion.span animate={{ rotate: 360 }} transition={{ duration: 1, repeat: Infinity, ease: 'linear' }}>
                              <RefreshIcon size={14} />
                            </motion.span>
                          ) : (
                            <>
                              <SendIcon size={14} />
                              Submit Proof
                            </>
                          )}
                        </button>
                      )}
                    </div>
                  </motion.div>
                ))}
                {verified > 0 && verified < total && (
                  <div className="portal__req-hint">
                    {total - verified} requirement{total - verified > 1 ? 's' : ''} remaining
                  </div>
                )}
                {verified === total && total > 0 && (
                  <motion.div
                    className="portal__all-done"
                    initial={{ opacity: 0, scale: 0.95 }}
                    animate={{ opacity: 1, scale: 1 }}
                  >
                    <CheckIcon size={20} />
                    <span>All requirements verified — </span>
                    <button className="portal__assess-link" onClick={handleTriggerAssessment}>
                      Trigger AI Assessment
                    </button>
                  </motion.div>
                )}
              </div>
            )}
          </motion.div>
        )}

        {activeTab === 'submissions' && (
          <motion.div
            key="submissions"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -12 }}
            transition={{ duration: 0.2 }}
            className="portal__panel"
          >
            {verified === 0 ? (
              <div className="portal__panel-empty">
                No submissions yet. Submit proofs in the Requirements tab.
              </div>
            ) : (
              <div className="portal__req-list">
                {requirements.filter(r => r.status === 'verified').map((req, i) => (
                  <motion.div
                    key={req.id}
                    className="portal__req portal__req--verified"
                    initial={{ opacity: 0, x: -16 }}
                    animate={{ opacity: 1, x: 0 }}
                    transition={{ delay: i * 0.05 }}
                  >
                    <div className="portal__req-icon"><CheckIcon size={16} /></div>
                    <div className="portal__req-info">
                      <h4 className="portal__req-type">{req.claim_type.replace(/_/g, ' ')}</h4>
                      <p className="portal__req-desc">Cryptographically verified inside T3N TEE</p>
                    </div>
                    <span className="portal__req-done">Verified</span>
                  </motion.div>
                ))}
              </div>
            )}
          </motion.div>
        )}

        {activeTab === 'assessment' && (
          <motion.div
            key="assessment"
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: -12 }}
            transition={{ duration: 0.2 }}
            className="portal__panel"
          >
            {assessmentLoading ? (
              <div className="portal__assessment portal__assessment--loading">
                <motion.div animate={{ rotate: 360 }} transition={{ duration: 2, repeat: Infinity, ease: 'linear' }}>
                  <RefreshIcon size={24} />
                </motion.div>
                <h3>AI Assessment Running</h3>
                <p>4-agent pipeline executing inside T3N TEE...</p>
                <div className="portal__pipeline-stages">
                  <span>Planner</span><span>→</span>
                  <span>Interpreter</span><span>→</span>
                  <span>Summarizer</span><span>→</span>
                  <span>Recommender</span>
                </div>
              </div>
            ) : assessment ? (
              <div className="portal__assessment portal__assessment--done">
                <div className="portal__assessment-header">
                  <div className={`portal__decision portal__decision--${assessment.decision.toLowerCase().replace(/ /g, '-')}`}>
                    {assessment.decision === 'Ready' && <CheckIcon size={20} />}
                    {assessment.decision === 'NeedsReview' && <ClockIcon size={20} />}
                    {assessment.decision === 'Blocked' && <AlertIcon size={20} />}
                    <span>{assessment.decision.replace(/([A-Z])/g, ' $1').trim()}</span>
                  </div>
                  <div className="portal__confidence">
                    <span className="portal__confidence-value">{Math.round((assessment.confidence || 0) * 100)}%</span>
                    <span className="portal__confidence-label">Confidence</span>
                  </div>
                </div>
                {assessment.summary_text && (
                  <div className="portal__assessment-summary">
                    <pre>{assessment.summary_text}</pre>
                  </div>
                )}
                <div className="portal__assessment-badge">
                  <ShieldIcon size={12} />
                  <span>Assessed inside T3N TEE — facts never left enclave</span>
                </div>
              </div>
            ) : (
              <div className="portal__assessment">
                <div className="portal__assessment-info">
                  <RefreshIcon size={24} />
                  <h3>AI Risk Assessment</h3>
                  <p>
                    {percentage >= 100
                      ? 'All requirements met. Ready for AI assessment.'
                      : `${percentage}% complete — submit remaining proofs to enable assessment.`}
                  </p>
                </div>
                {(verified > 0 || submittedIds.size > 0) && (
                  <button
                    className="portal__assess-btn"
                    onClick={handleTriggerAssessment}
                    disabled={assessmentLoading}
                  >
                    <ShieldIcon size={14} />
                    Trigger AI Assessment
                  </button>
                )}
              </div>
            )}
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
