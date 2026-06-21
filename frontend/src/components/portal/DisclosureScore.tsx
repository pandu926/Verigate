import './DisclosureScore.css';

interface DisclosureScoreProps {
  /** Number of fields actually disclosed across all submissions. */
  readonly disclosedClaims: number;
  /** Total available fields that could have been disclosed. */
  readonly totalAvailableClaims: number;
}

type PrivacyLevel = 'low' | 'medium' | 'high';

function getPrivacyLevel(ratio: number): PrivacyLevel {
  if (ratio <= 0.3) return 'low';
  if (ratio <= 0.6) return 'medium';
  return 'high';
}

/**
 * Privacy shield visualization showing the minimal disclosure ratio.
 * Lower disclosure = better privacy = stronger/greener shield.
 */
export function DisclosureScore({ disclosedClaims, totalAvailableClaims }: DisclosureScoreProps) {
  const ratio = totalAvailableClaims > 0 ? disclosedClaims / totalAvailableClaims : 0;
  const percentage = Math.round(ratio * 100);
  const level = getPrivacyLevel(ratio);

  // Shield opacity: more opaque (stronger) when disclosure is LOW (good)
  const shieldStrength = 1 - ratio * 0.6;

  return (
    <div className={`disclosure-score disclosure-score--${level}`}>
      <div className="disclosure-score__shield-wrapper">
        <svg
          className="disclosure-score__shield"
          width="56"
          height="64"
          viewBox="0 0 56 64"
          fill="none"
          aria-hidden="true"
          style={{ opacity: shieldStrength }}
        >
          {/* Shield outline */}
          <path
            d="M28 4L6 14v18c0 14.4 9.4 27.8 22 32 12.6-4.2 22-17.6 22-32V14L28 4z"
            className="disclosure-score__shield-fill"
          />
          <path
            d="M28 4L6 14v18c0 14.4 9.4 27.8 22 32 12.6-4.2 22-17.6 22-32V14L28 4z"
            className="disclosure-score__shield-stroke"
            strokeWidth="2"
            strokeLinejoin="round"
          />
          {/* Lock icon inside shield */}
          <rect x="22" y="28" width="12" height="10" rx="2" className="disclosure-score__lock-body" strokeWidth="1.5" />
          <path d="M24.5 28v-3a3.5 3.5 0 0 1 7 0v3" className="disclosure-score__lock-shackle" strokeWidth="1.5" strokeLinecap="round" />
          <circle cx="28" cy="33" r="1.5" className="disclosure-score__lock-keyhole" />
        </svg>

        {/* Glow effect behind shield for low disclosure */}
        {level === 'low' && (
          <div className="disclosure-score__glow" aria-hidden="true" />
        )}
      </div>

      <div className="disclosure-score__text">
        <span className="disclosure-score__label">Minimal Disclosure Score</span>
        <span className="disclosure-score__ratio">
          <span className="disclosure-score__number">{disclosedClaims}</span>
          {' of '}
          <span className="disclosure-score__number">{totalAvailableClaims}</span>
          {' fields disclosed'}
        </span>
        <span className={`disclosure-score__percentage disclosure-score__percentage--${level}`}>
          {percentage}%
        </span>
        <span className="disclosure-score__hint">Lower is better</span>
      </div>
    </div>
  );
}
