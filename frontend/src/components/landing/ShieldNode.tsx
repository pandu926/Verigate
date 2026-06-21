import { motion } from 'framer-motion';

export function ShieldNode() {
  return (
    <div className="shield-node">
      {/* Outer rotating ring */}
      <motion.svg
        className="shield-node__ring shield-node__ring--outer"
        viewBox="0 0 300 300"
        animate={{ rotate: 360 }}
        transition={{ duration: 60, repeat: Infinity, ease: 'linear' }}
      >
        <defs>
          <linearGradient id="ring-grad-outer" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#5eead4" stopOpacity="0.8" />
            <stop offset="50%" stopColor="#0d9488" stopOpacity="0.3" />
            <stop offset="100%" stopColor="#5eead4" stopOpacity="0.8" />
          </linearGradient>
        </defs>
        <circle cx="150" cy="150" r="140" fill="none" stroke="url(#ring-grad-outer)" strokeWidth="0.5" strokeDasharray="8 4" />
        {/* Data nodes on ring */}
        {[0, 60, 120, 180, 240, 300].map((angle) => {
          const rad = (angle * Math.PI) / 180;
          const x = 150 + 140 * Math.cos(rad);
          const y = 150 + 140 * Math.sin(rad);
          return (
            <circle key={angle} cx={x} cy={y} r="3" fill="#5eead4" opacity="0.7" />
          );
        })}
      </motion.svg>

      {/* Middle pulsing ring */}
      <motion.svg
        className="shield-node__ring shield-node__ring--mid"
        viewBox="0 0 300 300"
        animate={{ rotate: -360 }}
        transition={{ duration: 45, repeat: Infinity, ease: 'linear' }}
      >
        <defs>
          <linearGradient id="ring-grad-mid" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#2dd4bf" stopOpacity="0.6" />
            <stop offset="100%" stopColor="#0f766e" stopOpacity="0.2" />
          </linearGradient>
        </defs>
        <circle cx="150" cy="150" r="105" fill="none" stroke="url(#ring-grad-mid)" strokeWidth="1" strokeDasharray="3 6" />
        {[30, 90, 150, 210, 270, 330].map((angle) => {
          const rad = (angle * Math.PI) / 180;
          const x = 150 + 105 * Math.cos(rad);
          const y = 150 + 105 * Math.sin(rad);
          return (
            <rect key={angle} x={x - 2} y={y - 2} width="4" height="4" fill="#2dd4bf" opacity="0.5" transform={`rotate(45 ${x} ${y})`} />
          );
        })}
      </motion.svg>

      {/* Core shield icon */}
      <motion.svg
        className="shield-node__core"
        viewBox="0 0 64 64"
        initial={{ scale: 0.8, opacity: 0 }}
        animate={{ scale: 1, opacity: 1 }}
        transition={{ duration: 1, delay: 0.8 }}
      >
        <defs>
          <linearGradient id="shield-fill" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" stopColor="#14b8a6" />
            <stop offset="100%" stopColor="#0d9488" />
          </linearGradient>
          <filter id="shield-glow">
            <feGaussianBlur stdDeviation="2" result="blur" />
            <feComposite in="SourceGraphic" in2="blur" operator="over" />
          </filter>
        </defs>
        <path
          d="M32 4L8 16v16c0 14.4 10.2 27.8 24 32 13.8-4.2 24-17.6 24-32V16L32 4z"
          fill="url(#shield-fill)"
          opacity="0.15"
          filter="url(#shield-glow)"
        />
        <path
          d="M32 4L8 16v16c0 14.4 10.2 27.8 24 32 13.8-4.2 24-17.6 24-32V16L32 4z"
          fill="none"
          stroke="url(#shield-fill)"
          strokeWidth="1.5"
        />
        {/* Check mark inside shield */}
        <motion.path
          d="M22 32l6 6 14-14"
          fill="none"
          stroke="#5eead4"
          strokeWidth="2.5"
          strokeLinecap="round"
          strokeLinejoin="round"
          initial={{ pathLength: 0 }}
          animate={{ pathLength: 1 }}
          transition={{ duration: 0.8, delay: 1.5 }}
        />
      </motion.svg>

      {/* Ambient glow */}
      <div className="shield-node__glow" />
    </div>
  );
}
