import { type CSSProperties } from 'react';

type StatusLevel = 'healthy' | 'degraded' | 'offline';

interface StatusPulseProps {
  readonly status: StatusLevel;
  readonly label: string;
}

const STATUS_COLORS: Record<StatusLevel, string> = {
  healthy: 'var(--color-success)',
  degraded: 'var(--color-warning)',
  offline: 'var(--color-danger)',
};

const STATUS_GLOW: Record<StatusLevel, string> = {
  healthy: 'var(--color-success-dim)',
  degraded: 'var(--color-warning-dim)',
  offline: 'var(--color-danger-dim)',
};

/**
 * Animated status indicator with pulsing ring.
 * Uses compositor-friendly properties (transform, opacity) only.
 */
export function StatusPulse({ status, label }: StatusPulseProps) {
  const color = STATUS_COLORS[status];
  const glow = STATUS_GLOW[status];

  const containerStyle: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    gap: 'var(--space-sm)',
  };

  const indicatorWrapperStyle: CSSProperties = {
    position: 'relative',
    width: '10px',
    height: '10px',
  };

  const dotStyle: CSSProperties = {
    position: 'absolute',
    inset: 0,
    borderRadius: '50%',
    backgroundColor: color,
  };

  const ringStyle: CSSProperties = {
    position: 'absolute',
    inset: 0,
    borderRadius: '50%',
    backgroundColor: glow,
    animation:
      status === 'healthy'
        ? 'status-ring 2s var(--ease-out) infinite'
        : status === 'degraded'
          ? 'pulse-glow 1.5s ease-in-out infinite'
          : 'none',
  };

  const labelStyle: CSSProperties = {
    fontFamily: 'var(--font-mono)',
    fontSize: 'var(--text-xs)',
    color: 'var(--color-text-secondary)',
    letterSpacing: 'var(--tracking-wide)',
    textTransform: 'uppercase' as const,
  };

  return (
    <div style={containerStyle} role="status" aria-label={`System status: ${status}`}>
      <div style={indicatorWrapperStyle}>
        <span style={ringStyle} aria-hidden="true" />
        <span style={dotStyle} aria-hidden="true" />
      </div>
      <span style={labelStyle}>{label}</span>
    </div>
  );
}
