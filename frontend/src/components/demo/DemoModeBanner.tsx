import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { DEMO_STEPS } from '@/types/demo';
import './DemoModeBanner.css';

const DISMISSED_KEY = 'demo_banner_dismissed';

interface DemoModeBannerProps {
  readonly step: number;
}

/**
 * Contextual demo guidance banner.
 * Shows the current step in the 8-step demo narrative with navigation dots.
 * Dismissible (persisted per session via sessionStorage).
 */
export function DemoModeBanner({ step }: DemoModeBannerProps) {
  const navigate = useNavigate();
  const [dismissed, setDismissed] = useState(() => {
    return sessionStorage.getItem(DISMISSED_KEY) === 'true';
  });

  if (dismissed) {
    return null;
  }

  const currentIndex = Math.max(0, Math.min(step - 1, DEMO_STEPS.length - 1));
  const currentStep = DEMO_STEPS[currentIndex];

  function handleDismiss() {
    sessionStorage.setItem(DISMISSED_KEY, 'true');
    setDismissed(true);
  }

  function handleNav(direction: -1 | 1) {
    const nextIndex = currentIndex + direction;
    if (nextIndex < 0 || nextIndex >= DEMO_STEPS.length) return;
    const nextStep = DEMO_STEPS[nextIndex];
    if (nextStep) {
      navigate(nextStep.route.replace('__CASE_ID__', 'demo'));
    }
  }

  return (
    <div className="demo-banner" role="banner" aria-label="Demo mode navigation">
      <span className="demo-banner__badge">DEMO MODE</span>

      <div className="demo-banner__content">
        <p className="demo-banner__step-title">
          Step {currentStep?.step}: {currentStep?.title}
        </p>
        <p className="demo-banner__step-desc">{currentStep?.description}</p>
      </div>

      <div className="demo-banner__dots" aria-label={`Step ${step} of ${DEMO_STEPS.length}`}>
        {DEMO_STEPS.map((s, i) => {
          let className = 'demo-banner__dot';
          if (i === currentIndex) {
            className += ' demo-banner__dot--active';
          } else if (i < currentIndex) {
            className += ' demo-banner__dot--completed';
          }
          return <span key={s.step} className={className} />;
        })}
      </div>

      <div className="demo-banner__nav">
        <button
          type="button"
          className="demo-banner__arrow"
          onClick={() => handleNav(-1)}
          disabled={currentIndex === 0}
          aria-label="Previous step"
        >
          &#8249;
        </button>
        <button
          type="button"
          className="demo-banner__arrow"
          onClick={() => handleNav(1)}
          disabled={currentIndex === DEMO_STEPS.length - 1}
          aria-label="Next step"
        >
          &#8250;
        </button>
      </div>

      <button
        type="button"
        className="demo-banner__dismiss"
        onClick={handleDismiss}
        aria-label="Dismiss demo banner"
      >
        &#10005;
      </button>
    </div>
  );
}
