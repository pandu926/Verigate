import { useState } from 'react';
import { useParams, Link } from 'react-router-dom';
import { motion } from 'framer-motion';
import { FieldRedaction } from '@/components/demo/FieldRedaction';
import { AnimatedCounter } from '@/components/ui/AnimatedCounter';
import { ShieldIcon } from '@/components/ui/Icons';
import { FULL_CREDENTIALS } from '@/types/demo';
import type { FullCredential } from '@/types/demo';
import './PrivacySplitScreen.css';

const CREDENTIAL_TYPES = [
  'entity_registration',
  'authorized_signer',
  'jurisdiction_compliance',
  'beneficial_ownership',
] as const;

type CredentialType = (typeof CREDENTIAL_TYPES)[number];

const T3N_DID = 'did:t3n:ede53f4ac2149d9c6e663e47d5b5727ccd851e80';

export function PrivacySplitScreen() {
  const { caseId = '' } = useParams<{ caseId: string }>();
  const [activeTab, setActiveTab] = useState<CredentialType>('entity_registration');
  const [animKey, setAnimKey] = useState(0);

  const credential = FULL_CREDENTIALS[activeTab] as FullCredential;
  const fields = Object.entries(credential.fields);
  const disclosedFields = fields.filter(([, f]) => f.disclosed);
  const totalFields = fields.length;
  const disclosedCount = disclosedFields.length;
  const protectedCount = totalFields - disclosedCount;
  const minimization = Math.round((protectedCount / totalFields) * 100);

  function handleReplay() {
    setAnimKey((prev) => prev + 1);
  }

  function handleTabChange(tab: CredentialType) {
    setActiveTab(tab);
    setAnimKey((prev) => prev + 1);
  }

  return (
    <div>
      {/* Top controls */}
      <div className="split-screen__controls">
        <Link to="/dashboard" className="split-screen__back">
          &larr; Dashboard
        </Link>
        <button
          type="button"
          className="split-screen__replay"
          onClick={handleReplay}
        >
          &#8634; Replay Animation
        </button>
      </div>

      {/* Tab bar */}
      <div className="split-screen__tabs">
        {CREDENTIAL_TYPES.map((type) => (
          <button
            key={type}
            type="button"
            className={`split-screen__tab${activeTab === type ? ' split-screen__tab--active' : ''}`}
            onClick={() => handleTabChange(type)}
          >
            {(FULL_CREDENTIALS[type] as FullCredential).label}
          </button>
        ))}
      </div>

      {/* Split panels */}
      <div className="split-screen" key={animKey}>
        {/* Left: Full credentials */}
        <section
          className="split-screen__panel split-screen__panel--full"
          aria-label="Full credential data provided by counterparty"
        >
          <div className="split-screen__header">
            <span className="split-screen__header-icon split-screen__header-icon--full" aria-hidden="true">
              &#128275;
            </span>
            <h3 className="split-screen__title">What the Counterparty Provided</h3>
            <span className="split-screen__subtitle">Raw PII — sensitive data</span>
          </div>

          <dl className="split-screen__fields">
            {fields.map(([key, field], index) => (
              <FieldRedaction
                key={`${activeTab}-${key}`}
                fieldName={key.replace(/_/g, ' ')}
                value={field.value}
                disclosed={field.disclosed}
                animationDelay={index * 150}
              />
            ))}
          </dl>
        </section>

        {/* Center: TEE Shield Badge */}
        <div className="split-screen__tee-badge" aria-label="T3N TEE boundary">
          <motion.div
            className="tee-badge__shield"
            initial={{ scale: 0.8, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            transition={{ delay: 0.3, duration: 0.5, ease: [0.16, 1, 0.3, 1] }}
          >
            <ShieldIcon size={36} />
          </motion.div>
          <motion.div
            className="tee-badge__info"
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.6, duration: 0.4 }}
          >
            <span className="tee-badge__label">T3N TEE</span>
            <span className="tee-badge__sublabel">Selective Disclosure</span>
            <span className="tee-badge__did">{T3N_DID.slice(0, 12)}...{T3N_DID.slice(-4)}</span>
            <span className="tee-badge__status">
              <span className="tee-badge__pulse" />
              Enclave Active
            </span>
          </motion.div>
        </div>

        {/* Right: Disclosed only */}
        <section
          className="split-screen__panel split-screen__panel--disclosed"
          aria-label="Disclosed facts visible to the AI agent"
        >
          <div className="split-screen__header">
            <span className="split-screen__header-icon split-screen__header-icon--disclosed" aria-hidden="true">
              &#128737;
            </span>
            <h3 className="split-screen__title">What the AI Agent Sees</h3>
            <span className="split-screen__subtitle">Verified claims only — zero PII</span>
          </div>

          <dl className="split-screen__fields">
            {disclosedFields.map(([key, field], index) => (
              <FieldRedaction
                key={`${activeTab}-disclosed-${key}`}
                fieldName={key.replace(/_/g, ' ')}
                value={field.value}
                disclosed={true}
                animationDelay={index * 200 + 400}
              />
            ))}
            <div className="split-screen__tee-verified">
              <ShieldIcon size={12} />
              <span>All facts verified inside T3N TEE enclave</span>
            </div>
          </dl>
        </section>
      </div>

      {/* Bottom stats bar — three columns */}
      <motion.div
        className="split-screen__stats"
        initial={{ opacity: 0, y: 16 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ delay: 1.2, duration: 0.5 }}
      >
        <div className="split-screen__stat split-screen__stat--protected">
          <span className="split-screen__stat-value">
            <AnimatedCounter value={protectedCount} />
          </span>
          <span className="split-screen__stat-label">Fields Protected</span>
        </div>
        <div className="split-screen__stat split-screen__stat--disclosed">
          <span className="split-screen__stat-value">
            <AnimatedCounter value={disclosedCount} />
          </span>
          <span className="split-screen__stat-label">Facts Disclosed</span>
        </div>
        <div className="split-screen__stat split-screen__stat--minimization">
          <span className="split-screen__stat-value">
            <AnimatedCounter value={minimization} />%
          </span>
          <span className="split-screen__stat-label">Data Minimization</span>
        </div>
        {caseId && (
          <div className="split-screen__stat split-screen__stat--case">
            <span className="split-screen__stat-label">Case: {caseId.slice(0, 8)}</span>
          </div>
        )}
      </motion.div>
    </div>
  );
}
