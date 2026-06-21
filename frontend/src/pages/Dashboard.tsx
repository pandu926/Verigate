import { type CSSProperties } from 'react';
import { useHealthCheck } from '@/hooks/useHealthCheck';

/**
 * Dashboard page showing agent identity and system status.
 * This is the initial landing view displaying the health check data.
 */
export function Dashboard() {
  const { data, isLoading, isError, error } = useHealthCheck();

  const containerStyle: CSSProperties = {
    display: 'flex',
    flexDirection: 'column',
    gap: 'var(--space-xl)',
  };

  const headingStyle: CSSProperties = {
    fontSize: 'var(--text-3xl)',
    fontWeight: 300,
    letterSpacing: 'var(--tracking-tight)',
    color: 'var(--color-text-primary)',
    margin: 0,
    lineHeight: 'var(--leading-tight)',
  };

  const subtitleStyle: CSSProperties = {
    fontSize: 'var(--text-base)',
    color: 'var(--color-text-secondary)',
    margin: 0,
    marginTop: 'var(--space-sm)',
    maxWidth: '480px',
  };

  const cardStyle: CSSProperties = {
    background: 'var(--color-bg-tertiary)',
    border: '1px solid var(--color-border)',
    borderRadius: 'var(--radius-lg)',
    padding: 'var(--space-xl)',
    boxShadow: 'var(--shadow-md)',
    animation: 'fade-in var(--duration-slow) var(--ease-out) both',
    animationDelay: '150ms',
  };

  const cardHeaderStyle: CSSProperties = {
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: 'var(--space-lg)',
    paddingBottom: 'var(--space-md)',
    borderBottom: '1px solid var(--color-border)',
  };

  const cardTitleStyle: CSSProperties = {
    fontSize: 'var(--text-sm)',
    fontFamily: 'var(--font-mono)',
    fontWeight: 500,
    color: 'var(--color-text-secondary)',
    letterSpacing: 'var(--tracking-wide)',
    textTransform: 'uppercase',
    margin: 0,
  };

  const fieldGroupStyle: CSSProperties = {
    display: 'grid',
    gridTemplateColumns: 'minmax(140px, auto) 1fr',
    gap: `var(--space-sm) var(--space-lg)`,
    alignItems: 'baseline',
  };

  const labelStyle: CSSProperties = {
    fontSize: 'var(--text-xs)',
    fontFamily: 'var(--font-mono)',
    color: 'var(--color-text-tertiary)',
    letterSpacing: 'var(--tracking-wide)',
    textTransform: 'uppercase',
  };

  const valueStyle: CSSProperties = {
    fontSize: 'var(--text-sm)',
    color: 'var(--color-text-primary)',
    fontFamily: 'var(--font-mono)',
    wordBreak: 'break-all',
  };

  const statusBadgeStyle = (isActive: boolean): CSSProperties => ({
    display: 'inline-flex',
    alignItems: 'center',
    gap: 'var(--space-xs)',
    fontSize: 'var(--text-xs)',
    fontFamily: 'var(--font-mono)',
    fontWeight: 500,
    padding: `2px var(--space-sm)`,
    borderRadius: 'var(--radius-sm)',
    background: isActive
      ? 'oklch(72% 0.17 145 / 0.1)'
      : 'oklch(65% 0.2 25 / 0.1)',
    color: isActive ? 'var(--color-success)' : 'var(--color-danger)',
    border: `1px solid ${isActive ? 'oklch(72% 0.17 145 / 0.2)' : 'oklch(65% 0.2 25 / 0.2)'}`,
  });

  const capabilityChipStyle: CSSProperties = {
    display: 'inline-block',
    fontSize: 'var(--text-xs)',
    fontFamily: 'var(--font-mono)',
    padding: `2px var(--space-sm)`,
    borderRadius: 'var(--radius-sm)',
    background: 'var(--color-accent-glow)',
    color: 'var(--color-accent)',
    border: '1px solid oklch(72% 0.18 165 / 0.2)',
    marginRight: 'var(--space-xs)',
    marginBottom: 'var(--space-xs)',
  };

  const loadingStyle: CSSProperties = {
    color: 'var(--color-text-tertiary)',
    fontFamily: 'var(--font-mono)',
    fontSize: 'var(--text-sm)',
  };

  if (isLoading) {
    return (
      <div style={containerStyle}>
        <div>
          <h2 style={headingStyle}>Verigate</h2>
          <p style={subtitleStyle}>Autonomous counterparty assessment agent</p>
        </div>
        <div style={cardStyle}>
          <p style={loadingStyle}>Establishing connection...</p>
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div style={containerStyle}>
        <div>
          <h2 style={headingStyle}>Verigate</h2>
          <p style={subtitleStyle}>Autonomous counterparty assessment agent</p>
        </div>
        <div style={{ ...cardStyle, borderColor: 'oklch(65% 0.2 25 / 0.3)' }}>
          <h3 style={cardTitleStyle}>Connection Error</h3>
          <p style={{ ...valueStyle, color: 'var(--color-danger)' }}>
            {error?.message ?? 'Unable to reach backend service'}
          </p>
        </div>
      </div>
    );
  }

  return (
    <div style={containerStyle}>
      <div>
        <h2 style={headingStyle}>Verigate</h2>
        <p style={subtitleStyle}>Autonomous counterparty assessment agent</p>
      </div>

      {/* Agent Identity Card */}
      <div style={cardStyle}>
        <div style={cardHeaderStyle}>
          <h3 style={cardTitleStyle}>Agent Identity</h3>
          <span style={statusBadgeStyle(data?.agent_identity.authenticated ?? false)}>
            {data?.agent_identity.authenticated ? 'Authenticated' : 'Unauthenticated'}
          </span>
        </div>

        <div style={fieldGroupStyle}>
          <span style={labelStyle}>Agent DID</span>
          <span style={valueStyle}>{data?.agent_identity.agent_did}</span>

          <span style={labelStyle}>SDK Version</span>
          <span style={valueStyle}>{data?.agent_identity.sdk_version}</span>

          <span style={labelStyle}>Capabilities</span>
          <div>
            {data?.agent_identity.capabilities.map((cap) => (
              <span key={cap} style={capabilityChipStyle}>
                {cap}
              </span>
            ))}
          </div>
        </div>
      </div>

      {/* System Status Card */}
      <div style={{ ...cardStyle, animationDelay: '300ms' }}>
        <div style={cardHeaderStyle}>
          <h3 style={cardTitleStyle}>System Status</h3>
          <span style={statusBadgeStyle(data?.status === 'healthy')}>
            {data?.status}
          </span>
        </div>

        <div style={fieldGroupStyle}>
          <span style={labelStyle}>Version</span>
          <span style={valueStyle}>{data?.version}</span>

          <span style={labelStyle}>Database</span>
          <span style={statusBadgeStyle(data?.database_connected ?? false)}>
            {data?.database_connected ? 'Connected' : 'Disconnected'}
          </span>

          <span style={labelStyle}>Uptime</span>
          <span style={valueStyle}>
            {data ? formatUptime(data.uptime_seconds) : '—'}
          </span>
        </div>
      </div>
    </div>
  );
}

function formatUptime(seconds: number): string {
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.floor((seconds % 3600) / 60);
  const secs = Math.floor(seconds % 60);

  if (hours > 0) {
    return `${hours}h ${minutes}m ${secs}s`;
  }
  if (minutes > 0) {
    return `${minutes}m ${secs}s`;
  }
  return `${secs}s`;
}
