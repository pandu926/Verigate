import { useState, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { ScenarioSelector } from '@/components/evidence/ScenarioSelector';
import { TimelineStep } from '@/components/evidence/TimelineStep';
import { DelegationCard } from '@/components/evidence/DelegationCard';
import { DecisionBadge } from '@/components/evidence/DecisionBadge';
import './EvidenceChain.css';

const BRIDGE_URL = import.meta.env.VITE_BRIDGE_URL || '/bridge';

interface TimelineEntry {
  step: string;
  success: boolean;
  result: Record<string, unknown>;
  elapsed_ms: number;
}

interface PipelineResult {
  success: boolean;
  case_id: string;
  decision: string;
  confidence: number;
  evidence_chain_hash?: string;
  delegation: {
    vc_id: string;
    counterparty_did: string;
    agent_did: string;
    functions: string[];
    ttl_secs: number;
    revocation: { revoked: boolean; vc_id: string; revoked_at: string; reason: string };
    lifecycle: string;
  };
  timeline: TimelineEntry[];
  total_elapsed_ms: number;
  error?: string;
}

export function EvidenceChain() {
  const [isLoading, setIsLoading] = useState(false);
  const [activeScenario, setActiveScenario] = useState<string | null>(null);
  const [result, setResult] = useState<PipelineResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  const handleRun = useCallback(async (scenario: string) => {
    setIsLoading(true);
    setActiveScenario(scenario);
    setResult(null);
    setError(null);

    try {
      const res = await fetch(`${BRIDGE_URL}/scenarios/run`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ scenario }),
      });
      const data = await res.json() as PipelineResult;

      if (data.success) {
        setResult(data);
      } else {
        setError(data.error || 'Pipeline failed');
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Connection failed');
    } finally {
      setIsLoading(false);
    }
  }, []);

  const pipelineSteps = result?.timeline?.filter(
    t => !t.step.startsWith('delegation-')
  ) || [];
  const delegCreate = result?.timeline?.find(t => t.step === 'delegation-create');
  const delegRevoke = result?.timeline?.find(t => t.step === 'delegation-revoke');

  return (
    <div className="evidence-chain">
      <header className="evidence-chain__header">
        <h1 className="evidence-chain__title">Evidence Chain</h1>
        <p className="evidence-chain__subtitle">
          Live T3N TEE execution with delegation, AI reasoning, and cryptographic proof
        </p>
      </header>

      <ScenarioSelector
        onSelect={handleRun}
        isLoading={isLoading}
        activeScenario={activeScenario}
      />

      {isLoading && (
        <div className="evidence-chain__loading">
          <motion.div
            className="evidence-chain__loader"
            animate={{ rotate: 360 }}
            transition={{ repeat: Infinity, duration: 1.5, ease: 'linear' }}
          />
          <p>Executing pipeline in T3N TEE...</p>
          <p className="evidence-chain__loading-sub">
            Building credentials, delegating, verifying, AI assessing...
          </p>
        </div>
      )}

      {error && (
        <motion.div
          className="evidence-chain__error"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
        >
          <p>{error}</p>
          {error.includes('Rate limit') && (
            <p className="evidence-chain__error-hint">
              T3N testnet rate limit — wait 60s and try again
            </p>
          )}
        </motion.div>
      )}

      <AnimatePresence>
        {result && (
          <motion.div
            className="evidence-chain__timeline"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            transition={{ duration: 0.3 }}
          >
            <div className="evidence-chain__case-header">
              <span className="evidence-chain__case-id">Case: {result.case_id}</span>
              <span className="evidence-chain__elapsed">{result.total_elapsed_ms}ms total</span>
            </div>

            <div className="evidence-chain__connector" />

            {delegCreate && (
              <DelegationCard
                type="create"
                data={delegCreate.result}
                index={0}
              />
            )}

            <div className="evidence-chain__steps">
              {pipelineSteps.map((entry, i) => (
                <TimelineStep
                  key={entry.step}
                  step={entry.step}
                  success={entry.success}
                  result={entry.result}
                  elapsed_ms={entry.elapsed_ms}
                  index={i + 1}
                />
              ))}
            </div>

            {result.decision && (
              <DecisionBadge
                decision={result.decision}
                confidence={result.confidence}
                index={pipelineSteps.length + 1}
              />
            )}

            {delegRevoke && (
              <DelegationCard
                type="revoke"
                data={delegRevoke.result}
                index={pipelineSteps.length + 2}
              />
            )}

            <motion.div
              className="evidence-chain__footer"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              transition={{ delay: (pipelineSteps.length + 3) * 0.3 }}
            >
              <div className="evidence-chain__footer-row">
                <span>Chain Hash</span>
                <span className="evidence-chain__mono">
                  {result.evidence_chain_hash?.slice(0, 24) || 'computed'}...
                </span>
              </div>
              <div className="evidence-chain__footer-row">
                <span>Execution</span>
                <span className="evidence-chain__tee-badge">T3N TEE Enclave</span>
              </div>
              <div className="evidence-chain__footer-row">
                <span>Delegation</span>
                <span>{result.delegation?.lifecycle}</span>
              </div>
            </motion.div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
