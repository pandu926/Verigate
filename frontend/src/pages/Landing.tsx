import { useRef } from 'react';
import { useNavigate } from 'react-router-dom';
import { motion, useScroll, useTransform, useInView } from 'framer-motion';
import { ShieldNode } from '@/components/landing/ShieldNode';
import { FlowDiagram } from '@/components/landing/FlowDiagram';
import { FeatureCard } from '@/components/landing/FeatureCard';
import { ParticleField } from '@/components/landing/ParticleField';
import '@/styles/landing.css';

const FEATURES = [
  {
    title: 'Selective Disclosure',
    description: 'Counterparties prove claims without exposing raw documents. SD-JWT credentials reveal only what policy demands.',
    icon: 'disclosure',
  },
  {
    title: 'Multi-Agent AI Pipeline',
    description: 'Four specialized agents chain reasoning — planner, interpreter, summarizer, recommender — operating only on verified facts.',
    icon: 'pipeline',
  },
  {
    title: 'TEE-Protected Actions',
    description: 'Sensitive operations execute inside Terminal 3 enclaves. PII placeholders resolve host-side — your server never touches raw data.',
    icon: 'tee',
  },
  {
    title: 'Cedar Policy Engine',
    description: 'Authorization as code. Fine-grained attribute-based access control with formally verified evaluation.',
    icon: 'policy',
  },
  {
    title: 'Verifiable Credentials',
    description: 'W3C-standard cryptographic proofs. Trusted issuer registry. Entity, signer, region, and wallet verification built in.',
    icon: 'credential',
  },
  {
    title: 'Full Audit Timeline',
    description: 'Every action identity-bound and immutable. Real-time SSE streaming. Append-only event log for compliance.',
    icon: 'audit',
  },
] as const;

export function Landing() {
  const navigate = useNavigate();
  const heroRef = useRef<HTMLDivElement>(null);
  const featuresRef = useRef<HTMLDivElement>(null);
  const ctaRef = useRef<HTMLDivElement>(null);
  const featuresInView = useInView(featuresRef, { once: true, margin: '-100px' });
  const ctaInView = useInView(ctaRef, { once: true, margin: '-80px' });

  const { scrollYProgress } = useScroll({
    target: heroRef,
    offset: ['start start', 'end start'],
  });

  const heroY = useTransform(scrollYProgress, [0, 1], [0, 150]);
  const heroOpacity = useTransform(scrollYProgress, [0, 0.8], [1, 0]);

  return (
    <div className="landing-root">
      <ParticleField />

      {/* Hero */}
      <section ref={heroRef} className="landing-hero">
        <motion.div
          className="landing-hero__content"
          style={{ y: heroY, opacity: heroOpacity }}
        >
          <motion.div
            className="landing-hero__badge"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6, delay: 0.2 }}
          >
            <span className="landing-hero__badge-dot" />
            Terminal 3 Agent Dev Kit
          </motion.div>

          <motion.h1
            className="landing-hero__title"
            initial={{ opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.8, delay: 0.4 }}
          >
            Autonomous
            <br />
            <span className="landing-hero__title-accent">Counterparty</span>
            <br />
            Verification
          </motion.h1>

          <motion.p
            className="landing-hero__subtitle"
            initial={{ opacity: 0, y: 30 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.7, delay: 0.7 }}
          >
            AI-driven due diligence that never sees your private data.
            <br />
            Verifiable credentials. Selective disclosure. TEE-protected execution.
          </motion.p>

          <motion.div
            className="landing-hero__actions"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6, delay: 1.0 }}
          >
            <button
              className="landing-btn landing-btn--primary"
              onClick={() => navigate('/dashboard')}
            >
              Launch App
              <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
                <path d="M3 8h10M9 4l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
              </svg>
            </button>
            <button
              className="landing-btn landing-btn--ghost"
              onClick={() => featuresRef.current?.scrollIntoView({ behavior: 'smooth' })}
            >
              Explore Features
            </button>
          </motion.div>

          <motion.div
            className="landing-hero__visual"
            initial={{ opacity: 0, scale: 0.9 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={{ duration: 1.2, delay: 0.6 }}
          >
            <ShieldNode />
          </motion.div>
        </motion.div>
      </section>

      {/* Trust Flow Section */}
      <section className="landing-flow">
        <motion.div
          className="landing-flow__inner"
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          transition={{ duration: 0.8 }}
          viewport={{ once: true, margin: '-100px' }}
        >
          <h2 className="landing-section-title">How Verigate Works</h2>
          <p className="landing-section-subtitle">
            A trust pipeline where AI reasons over proofs — not raw documents
          </p>
          <FlowDiagram />
        </motion.div>
      </section>

      {/* Features Grid */}
      <section ref={featuresRef} className="landing-features">
        <h2 className="landing-section-title">Built for Zero-Trust Compliance</h2>
        <p className="landing-section-subtitle">
          Every component designed around the principle: prove without exposing
        </p>
        <div className="landing-features__grid">
          {FEATURES.map((feature, i) => (
            <motion.div
              key={feature.title}
              initial={{ opacity: 0, y: 40 }}
              animate={featuresInView ? { opacity: 1, y: 0 } : {}}
              transition={{ duration: 0.5, delay: i * 0.1 }}
            >
              <FeatureCard
                title={feature.title}
                description={feature.description}
                icon={feature.icon}
              />
            </motion.div>
          ))}
        </div>
      </section>

      {/* CTA */}
      <section ref={ctaRef} className="landing-cta">
        <motion.div
          className="landing-cta__inner"
          initial={{ opacity: 0, y: 40 }}
          animate={ctaInView ? { opacity: 1, y: 0 } : {}}
          transition={{ duration: 0.7 }}
        >
          <h2 className="landing-cta__title">
            Ready to verify without compromise?
          </h2>
          <p className="landing-cta__text">
            Onboard counterparties autonomously. Full compliance. Zero PII exposure.
          </p>
          <button
            className="landing-btn landing-btn--primary landing-btn--lg"
            onClick={() => navigate('/dashboard')}
          >
            Enter Verigate
            <svg width="18" height="18" viewBox="0 0 16 16" fill="none">
              <path d="M3 8h10M9 4l4 4-4 4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round"/>
            </svg>
          </button>
        </motion.div>
      </section>

      {/* Footer */}
      <footer className="landing-footer">
        <div className="landing-footer__inner">
          <span className="landing-footer__brand">Verigate</span>
          <span className="landing-footer__copy">Autonomous Counterparty Agent</span>
          <span className="landing-footer__t3">Powered by Terminal 3 Trust Layer</span>
        </div>
      </footer>
    </div>
  );
}
