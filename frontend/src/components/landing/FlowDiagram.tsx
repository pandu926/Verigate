import { motion } from 'framer-motion';

const STEPS = [
  {
    label: 'Submit',
    detail: 'Counterparty presents verifiable credentials',
    icon: (
      <path d="M4 6h16M4 12h16M4 18h8" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    ),
  },
  {
    label: 'Verify',
    detail: 'Cryptographic proof validation + issuer trust check',
    icon: (
      <path d="M9 12l2 2 4-4M12 2a10 10 0 110 20 10 10 0 010-20z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    ),
  },
  {
    label: 'Disclose',
    detail: 'SD-JWT reveals only policy-required fields as facts',
    icon: (
      <>
        <rect x="3" y="3" width="18" height="18" rx="3" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <path d="M8 8h8M8 12h5M8 16h3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </>
    ),
  },
  {
    label: 'Assess',
    detail: 'Multi-agent AI pipeline reasons over disclosed facts only',
    icon: (
      <>
        <circle cx="12" cy="8" r="3" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <circle cx="6" cy="18" r="2.5" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <circle cx="18" cy="18" r="2.5" stroke="currentColor" strokeWidth="1.5" fill="none" />
        <path d="M12 11v2M9 15l-2 1.5M15 15l2 1.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      </>
    ),
  },
  {
    label: 'Decide',
    detail: 'Recommendation + human override with TEE-protected execution',
    icon: (
      <path d="M12 2L4 7v5c0 7 3.4 13.2 8 16 4.6-2.8 8-9 8-16V7l-8-5z" stroke="currentColor" strokeWidth="1.5" fill="none" strokeLinecap="round" strokeLinejoin="round" />
    ),
  },
];

export function FlowDiagram() {
  return (
    <div className="flow-diagram">
      {STEPS.map((step, i) => (
        <motion.div
          key={step.label}
          className="flow-diagram__step"
          initial={{ opacity: 0, x: -30 }}
          whileInView={{ opacity: 1, x: 0 }}
          transition={{ duration: 0.5, delay: i * 0.15 }}
          viewport={{ once: true, margin: '-50px' }}
        >
          <div className="flow-diagram__icon">
            <svg width="24" height="24" viewBox="0 0 24 24" fill="none">
              {step.icon}
            </svg>
          </div>
          <div className="flow-diagram__content">
            <span className="flow-diagram__number">{String(i + 1).padStart(2, '0')}</span>
            <h3 className="flow-diagram__label">{step.label}</h3>
            <p className="flow-diagram__detail">{step.detail}</p>
          </div>
          {i < STEPS.length - 1 && (
            <motion.div
              className="flow-diagram__connector"
              initial={{ scaleX: 0 }}
              whileInView={{ scaleX: 1 }}
              transition={{ duration: 0.4, delay: i * 0.15 + 0.3 }}
              viewport={{ once: true }}
            />
          )}
        </motion.div>
      ))}
    </div>
  );
}
