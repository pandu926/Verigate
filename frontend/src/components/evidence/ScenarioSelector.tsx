import { motion } from 'framer-motion';

interface ScenarioSelectorProps {
  onSelect: (scenario: string) => void;
  isLoading: boolean;
  activeScenario: string | null;
}

const SCENARIOS = [
  {
    id: 'good',
    label: 'Good Entity',
    subtitle: 'Singapore, Licensed, AML Clear',
    icon: '🏢',
    color: 'var(--color-success)',
  },
  {
    id: 'sanctioned',
    label: 'Sanctioned Entity',
    subtitle: 'North Korea, OFAC, UN Sanctions',
    icon: '🚫',
    color: 'var(--color-danger)',
  },
  {
    id: 'incomplete',
    label: 'Shell Company',
    subtitle: 'Unknown Jurisdiction, Nominee Structure',
    icon: '❓',
    color: 'var(--color-warning)',
  },
] as const;

export function ScenarioSelector({ onSelect, isLoading, activeScenario }: ScenarioSelectorProps) {
  return (
    <div className="scenario-selector">
      <h2 className="scenario-selector__title">Run Live Pipeline</h2>
      <p className="scenario-selector__subtitle">
        Select a scenario to execute against T3N TEE testnet
      </p>
      <div className="scenario-selector__grid">
        {SCENARIOS.map((s) => (
          <motion.button
            key={s.id}
            className={`scenario-card${activeScenario === s.id ? ' scenario-card--active' : ''}`}
            style={{ '--scenario-color': s.color } as React.CSSProperties}
            onClick={() => onSelect(s.id)}
            disabled={isLoading}
            whileHover={{ scale: 1.02 }}
            whileTap={{ scale: 0.98 }}
          >
            <span className="scenario-card__icon">{s.icon}</span>
            <span className="scenario-card__label">{s.label}</span>
            <span className="scenario-card__subtitle">{s.subtitle}</span>
          </motion.button>
        ))}
      </div>
    </div>
  );
}
