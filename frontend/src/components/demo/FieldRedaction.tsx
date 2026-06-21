import './FieldRedaction.css';

interface FieldRedactionProps {
  readonly fieldName: string;
  readonly value: string;
  readonly disclosed: boolean;
  readonly animationDelay: number;
}

/**
 * Animated field redaction component.
 * Disclosed fields show with a green checkmark and subtle glow.
 * Redacted fields animate: visible -> blur -> replaced with {{redacted}}.
 */
export function FieldRedaction({
  fieldName,
  value,
  disclosed,
  animationDelay,
}: FieldRedactionProps) {
  const delayStyle = { '--delay': `${animationDelay}ms` } as React.CSSProperties;

  if (disclosed) {
    return (
      <div
        className="field-redaction"
        aria-label={`${fieldName}: ${value} (disclosed to AI)`}
      >
        <dt className="field-redaction__key">{fieldName}</dt>
        <dd className="field-redaction__value--disclosed" style={delayStyle}>
          <span className="field-redaction__check" aria-hidden="true">
            &#10003;
          </span>
          {value}
        </dd>
      </div>
    );
  }

  return (
    <div
      className="field-redaction"
      aria-label={`${fieldName}: redacted (not disclosed to AI)`}
    >
      <dt className="field-redaction__key">{fieldName}</dt>
      <dd className="field-redaction__value--redacted">
        <span className="field-redaction__original" style={delayStyle}>
          {value}
        </span>
        <span
          className="field-redaction__placeholder"
          style={delayStyle}
          aria-hidden="true"
        >
          {'{{redacted}}'}
        </span>
      </dd>
    </div>
  );
}
