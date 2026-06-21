import { type CSSProperties } from 'react';
import './ProgressRing.css';

interface ProgressRingProps {
  /** Percentage complete (0–100). */
  readonly percentage: number;
  /** Number of verified items. */
  readonly verified: number;
  /** Total number of items. */
  readonly total: number;
}

const RING_SIZE = 160;
const STROKE_WIDTH = 6;
const RADIUS = (RING_SIZE - STROKE_WIDTH) / 2;
const CIRCUMFERENCE = 2 * Math.PI * RADIUS;

function getProgressTier(percentage: number): string {
  if (percentage >= 75) return 'progress-ring__fill--high';
  if (percentage >= 40) return 'progress-ring__fill--mid';
  return 'progress-ring__fill--low';
}

/**
 * Circular SVG progress indicator with animated stroke transition.
 * Shows percentage in center, fraction text below.
 */
export function ProgressRing({ percentage, verified, total }: ProgressRingProps) {
  const offset = CIRCUMFERENCE - (percentage / 100) * CIRCUMFERENCE;

  const wrapperStyle: CSSProperties = {
    position: 'relative',
    width: RING_SIZE,
    height: RING_SIZE,
  };

  return (
    <div className="progress-ring">
      <div style={wrapperStyle}>
        <svg
          className="progress-ring__svg"
          width={RING_SIZE}
          height={RING_SIZE}
          viewBox={`0 0 ${RING_SIZE} ${RING_SIZE}`}
          aria-label={`Progress: ${Math.round(percentage)}% complete`}
          role="img"
        >
          <circle
            className="progress-ring__track"
            cx={RING_SIZE / 2}
            cy={RING_SIZE / 2}
            r={RADIUS}
          />
          <circle
            className={`progress-ring__fill ${getProgressTier(percentage)}`}
            cx={RING_SIZE / 2}
            cy={RING_SIZE / 2}
            r={RADIUS}
            strokeDasharray={CIRCUMFERENCE}
            strokeDashoffset={offset}
          />
        </svg>
        <div className="progress-ring__center">
          <span className="progress-ring__percentage">
            {Math.round(percentage)}%
          </span>
          <span className="progress-ring__label">complete</span>
        </div>
      </div>
      <span className="progress-ring__fraction">
        {verified} of {total} verified
      </span>
    </div>
  );
}
