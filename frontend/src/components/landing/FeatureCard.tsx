import { motion } from 'framer-motion';

interface FeatureCardProps {
  title: string;
  description: string;
  icon: string;
}

const ICONS: Record<string, React.ReactNode> = {
  disclosure: (
    <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
      <rect x="4" y="6" width="24" height="20" rx="3" stroke="currentColor" strokeWidth="1.5" />
      <path d="M4 12h24" stroke="currentColor" strokeWidth="1.5" />
      <rect x="8" y="16" width="8" height="2" rx="1" fill="currentColor" opacity="0.6" />
      <rect x="8" y="20" width="5" height="2" rx="1" fill="currentColor" opacity="0.3" />
      <rect x="20" y="16" width="4" height="6" rx="1" stroke="currentColor" strokeWidth="1" strokeDasharray="2 1" />
    </svg>
  ),
  pipeline: (
    <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
      <circle cx="8" cy="16" r="3" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="16" cy="8" r="2.5" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="16" cy="24" r="2.5" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="24" cy="16" r="3" stroke="currentColor" strokeWidth="1.5" />
      <path d="M11 15l3-5M11 17l3 5M18.5 9.5l3 4.5M18.5 22.5l3-4.5" stroke="currentColor" strokeWidth="1" opacity="0.5" />
    </svg>
  ),
  tee: (
    <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
      <rect x="6" y="6" width="20" height="20" rx="4" stroke="currentColor" strokeWidth="1.5" />
      <rect x="10" y="10" width="12" height="12" rx="2" stroke="currentColor" strokeWidth="1" strokeDasharray="2 2" />
      <path d="M16 13v6M13 16h6" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
    </svg>
  ),
  policy: (
    <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
      <path d="M6 8h20M6 14h14M6 20h18M6 26h10" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <circle cx="26" cy="20" r="4" stroke="currentColor" strokeWidth="1.5" />
      <path d="M24.5 20l1 1 2.5-2.5" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  ),
  credential: (
    <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
      <rect x="4" y="8" width="24" height="16" rx="3" stroke="currentColor" strokeWidth="1.5" />
      <circle cx="12" cy="16" r="3" stroke="currentColor" strokeWidth="1.2" />
      <path d="M18 14h6M18 18h4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      <path d="M12 19v2" stroke="currentColor" strokeWidth="1" strokeLinecap="round" opacity="0.5" />
    </svg>
  ),
  audit: (
    <svg width="32" height="32" viewBox="0 0 32 32" fill="none">
      <path d="M8 6v20M8 6l4 3M8 6l-4 3" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
      <rect x="12" y="10" width="14" height="3" rx="1.5" stroke="currentColor" strokeWidth="1" />
      <rect x="12" y="16" width="14" height="3" rx="1.5" stroke="currentColor" strokeWidth="1" opacity="0.7" />
      <rect x="12" y="22" width="14" height="3" rx="1.5" stroke="currentColor" strokeWidth="1" opacity="0.4" />
    </svg>
  ),
};

export function FeatureCard({ title, description, icon }: FeatureCardProps) {
  return (
    <motion.div
      className="feature-card"
      whileHover={{ y: -4, transition: { duration: 0.2 } }}
    >
      <div className="feature-card__icon">
        {ICONS[icon]}
      </div>
      <h3 className="feature-card__title">{title}</h3>
      <p className="feature-card__desc">{description}</p>
      <div className="feature-card__shine" />
    </motion.div>
  );
}
