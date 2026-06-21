import { motion } from 'framer-motion';

interface DecisionBadgeProps {
  decision: string;
  confidence: number;
  index: number;
}

export function DecisionBadge({ decision, confidence, index }: DecisionBadgeProps) {
  const config = {
    approved: { color: 'var(--color-success)', glow: '0 0 40px oklch(72% 0.17 145 / 0.3)', label: 'APPROVED' },
    blocked: { color: 'var(--color-danger)', glow: '0 0 40px oklch(65% 0.2 25 / 0.3)', label: 'BLOCKED' },
    needs_review: { color: 'var(--color-warning)', glow: '0 0 40px oklch(78% 0.15 75 / 0.3)', label: 'NEEDS REVIEW' },
  }[decision] || { color: 'var(--color-text-secondary)', glow: 'none', label: decision?.toUpperCase() || 'UNKNOWN' };

  return (
    <motion.div
      className="decision-badge"
      style={{ '--decision-color': config.color, '--decision-glow': config.glow } as React.CSSProperties}
      initial={{ opacity: 0, scale: 0.8 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ delay: index * 0.3, duration: 0.6, ease: [0.16, 1, 0.3, 1] }}
    >
      <motion.div
        className="decision-badge__inner"
        animate={{ boxShadow: [config.glow, config.glow.replace('0.3', '0.6'), config.glow] }}
        transition={{ repeat: Infinity, duration: 2, ease: 'easeInOut' }}
      >
        <span className="decision-badge__label">{config.label}</span>
        <span className="decision-badge__confidence">
          Confidence: {(confidence * 100).toFixed(0)}%
        </span>
      </motion.div>
    </motion.div>
  );
}
