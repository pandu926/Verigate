import { useState, useEffect } from 'react';
import { useParams, Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import { ShieldIcon, CheckIcon, ClockIcon, ChevronRightIcon } from '@/components/ui/Icons';
import { AnimatedCounter } from '@/components/ui/AnimatedCounter';
import { addToast, ToastContainer } from '@/components/ui/Toast';
import { useRequirements } from '@/hooks/useRequirements';
import { triggerAssessment, fetchAssessment } from '@/lib/api';
import './CaseDetail.css';

const API_BASE = import.meta.env.VITE_API_URL || '';

interface AssessmentResult {
  decision: string;
  confidence: number;
  summary_text: string;
  agent_outputs?: {
    tee_mode?: string;
    delegation?: { vc_id?: string; counterparty_did?: string };
    timeline?: Array<{ step: string; result?: Record<string, unknown> }>;
  };
  created_at: string;
}

export function CaseDetail() {
  const { caseId = '' } = useParams<{ caseId: string }>();
  const [assessment, setAssessment] = useState<AssessmentResult | null>(null);
  const [assessing, setAssessing] = useState(false);
  const [caseInfo, setCaseInfo] = useState<Record<string, unknown> | null>(null);

  const { data: reqData } = useRequirements(caseId);
  const requirements = reqData?.data || [];
  const verified = requirements.filter((r: { status: string }) => r.status === 'verified').length;
  const total = requirements.length;

  useEffect(() => {
    loadCaseInfo();
    loadAssessment();
  }, [caseId]);

  const loadCaseInfo = async () => {
    try {
      const token = localStorage.getItem('verigate_token');
      const res = await fetch(`${API_BASE}/api/cases/${caseId}`, {
        headers: { 'Authorization': `Bearer ${token}` },
      });
      if (res.ok) setCaseInfo(await res.json().then(d => d.data || d));
    } catch { /* ignore */ }
  };

  const loadAssessment = async () => {
    try {
      const result = await fetchAssessment(caseId) as { data: AssessmentResult };
      if (result?.data) setAssessment(result.data);
    } catch { /* no assessment yet */ }
  };

  const handleTriggerAssessment = async () => {
    setAssessing(true);
    try {
      await triggerAssessment(caseId);
      addToast('success', 'Assessment triggered — running in T3N TEE');
      // Poll for result
      let attempts = 0;
      const poll = setInterval(async () => {
        attempts++;
        try {
          const result = await fetchAssessment(caseId) as { data: AssessmentResult };
          if (result?.data) {
            setAssessment(result.data);
            setAssessing(false);
            clearInterval(poll);
            addToast('success', `Decision: ${result.data.decision}`);
          }
        } catch { /* keep polling */ }
        if (attempts > 20) { clearInterval(poll); setAssessing(false); }
      }, 3000);
    } catch (e) {
      addToast('error', 'Assessment trigger failed');
      setAssessing(false);
    }
  };

  const decisionColor = (d: string) => {
    if (d === 'Ready' || d === 'ready' || d === 'approved') return 'var(--color-success)';
    if (d === 'Blocked' || d === 'blocked') return 'var(--color-danger)';
    return 'var(--color-warning)';
  };

  return (
    <div className="case-detail">
      <ToastContainer />
      <div className="case-detail__header">
        <Link to="/dashboard" className="case-detail__back">&larr; Dashboard</Link>
        <h1 className="case-detail__title">Case Review</h1>
        <span className="case-detail__id">{caseId.slice(0, 8)}...</span>
      </div>

      {/* Case Info */}
      {caseInfo && (
        <motion.section
          className="case-detail__info"
          initial={{ opacity: 0, y: 10 }}
          animate={{ opacity: 1, y: 0 }}
        >
          <div className="case-detail__info-row">
            <span className="case-detail__info-label">Entity</span>
            <span className="case-detail__info-value">{String(caseInfo.entity_name || 'Unknown')}</span>
          </div>
          <div className="case-detail__info-row">
            <span className="case-detail__info-label">Status</span>
            <span className="case-detail__info-value case-detail__status">{String(caseInfo.status || 'created')}</span>
          </div>
          <div className="case-detail__info-row">
            <span className="case-detail__info-label">Jurisdiction</span>
            <span className="case-detail__info-value">{String(caseInfo.jurisdiction || '-')}</span>
          </div>
          <div className="case-detail__info-row">
            <span className="case-detail__info-label">Workflow</span>
            <span className="case-detail__info-value">{String(caseInfo.workflow_type || '-')}</span>
          </div>
        </motion.section>
      )}

      {/* Requirements Progress */}
      <motion.section
        className="case-detail__section"
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.1 }}
      >
        <h2 className="case-detail__section-title">
          <CheckIcon size={18} /> Credential Submissions
        </h2>
        <div className="case-detail__progress">
          <div className="case-detail__progress-bar">
            <div className="case-detail__progress-fill" style={{ width: `${total > 0 ? (verified / total) * 100 : 0}%` }} />
          </div>
          <span className="case-detail__progress-text">
            <AnimatedCounter value={verified} /> / {total} verified
          </span>
        </div>
        <div className="case-detail__reqs">
          {requirements.map((r: { claim_type: string; status: string; description: string }, i: number) => (
            <div key={i} className={`case-detail__req ${r.status === 'verified' ? 'case-detail__req--verified' : ''}`}>
              {r.status === 'verified' ? <CheckIcon size={16} /> : <ClockIcon size={16} />}
              <span className="case-detail__req-type">{r.claim_type.replace(/_/g, ' ')}</span>
              <span className="case-detail__req-status">{r.status}</span>
            </div>
          ))}
        </div>
      </motion.section>

      {/* Assessment */}
      <motion.section
        className="case-detail__section"
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.2 }}
      >
        <h2 className="case-detail__section-title">
          <ShieldIcon size={18} /> TEE Assessment
        </h2>

        {assessment ? (
          <div className="case-detail__assessment">
            <div className="case-detail__decision" style={{ borderColor: decisionColor(assessment.decision) }}>
              <span className="case-detail__decision-label" style={{ color: decisionColor(assessment.decision) }}>
                {assessment.decision.toUpperCase()}
              </span>
              <span className="case-detail__decision-confidence">
                Confidence: {(assessment.confidence * 100).toFixed(0)}%
              </span>
            </div>
            <div className="case-detail__summary">
              {assessment.summary_text}
            </div>
            {assessment.agent_outputs?.tee_mode && (
              <div className="case-detail__tee-badge">
                <ShieldIcon size={14} />
                Executed in T3N TEE ({assessment.agent_outputs.tee_mode})
              </div>
            )}
            {assessment.agent_outputs?.delegation?.vc_id && (
              <div className="case-detail__delegation">
                <span>Delegation: {assessment.agent_outputs.delegation.vc_id.slice(0, 16)}...</span>
                <span>Counterparty: {assessment.agent_outputs.delegation.counterparty_did?.slice(0, 20)}...</span>
              </div>
            )}
          </div>
        ) : (
          <div className="case-detail__no-assessment">
            <p>{verified > 0 ? 'Credentials submitted. Ready for assessment.' : 'Waiting for counterparty to submit credentials.'}</p>
            <button
              className="case-detail__assess-btn"
              onClick={handleTriggerAssessment}
              disabled={assessing || verified === 0}
            >
              {assessing ? (
                <>Assessing in TEE...</>
              ) : (
                <>
                  <ShieldIcon size={16} />
                  Trigger AI Assessment
                </>
              )}
            </button>
          </div>
        )}
      </motion.section>

      {/* Actions */}
      <motion.section
        className="case-detail__actions"
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 0.3 }}
      >
        <Link to={`/evidence`} className="case-detail__action-btn">
          Evidence Chain <ChevronRightIcon size={14} />
        </Link>
        <Link to={`/privacy/${caseId}`} className="case-detail__action-btn case-detail__action-btn--ghost">
          Privacy View <ChevronRightIcon size={14} />
        </Link>
      </motion.section>
    </div>
  );
}
