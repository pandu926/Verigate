import { motion } from 'framer-motion';

interface DelegationCardProps {
  type: 'create' | 'revoke';
  data: {
    vc_id?: string;
    counterparty_did?: string;
    agent_did?: string;
    functions?: string[];
    ttl_secs?: number;
    revoked_at?: string;
    reason?: string;
  };
  index: number;
}

export function DelegationCard({ type, data, index }: DelegationCardProps) {
  const isCreate = type === 'create';

  return (
    <motion.div
      className={`delegation-card delegation-card--${type}`}
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ delay: index * 0.3, duration: 0.5 }}
    >
      <div className="delegation-card__header">
        <span className="delegation-card__icon">{isCreate ? '🔐' : '🔓'}</span>
        <span className="delegation-card__title">
          {isCreate ? 'Delegation Created' : 'Delegation Revoked'}
        </span>
      </div>
      <div className="delegation-card__body">
        {isCreate && (
          <>
            <div className="delegation-card__row">
              <span className="delegation-card__label">Data Owner</span>
              <span className="delegation-card__value delegation-card__value--mono">
                {data.counterparty_did?.slice(0, 20)}...
              </span>
            </div>
            <div className="delegation-card__row">
              <span className="delegation-card__label">Agent</span>
              <span className="delegation-card__value delegation-card__value--mono">
                {data.agent_did?.slice(0, 20)}...
              </span>
            </div>
            <div className="delegation-card__row">
              <span className="delegation-card__label">VC ID</span>
              <span className="delegation-card__value delegation-card__value--mono">
                {data.vc_id?.slice(0, 16)}...
              </span>
            </div>
            <div className="delegation-card__row">
              <span className="delegation-card__label">Scoped To</span>
              <span className="delegation-card__value">
                {data.functions?.length} functions
              </span>
            </div>
            <div className="delegation-card__row">
              <span className="delegation-card__label">TTL</span>
              <span className="delegation-card__value">{data.ttl_secs}s</span>
            </div>
            <div className="delegation-card__row">
              <span className="delegation-card__label">Signature</span>
              <span className="delegation-card__value delegation-card__value--accent">EIP-191</span>
            </div>
          </>
        )}
        {!isCreate && (
          <>
            <div className="delegation-card__row">
              <span className="delegation-card__label">VC ID</span>
              <span className="delegation-card__value delegation-card__value--mono">
                {data.vc_id?.slice(0, 16)}...
              </span>
            </div>
            <div className="delegation-card__row">
              <span className="delegation-card__label">Reason</span>
              <span className="delegation-card__value">{data.reason}</span>
            </div>
            <div className="delegation-card__row">
              <span className="delegation-card__label">Revoked At</span>
              <span className="delegation-card__value delegation-card__value--mono">
                {data.revoked_at}
              </span>
            </div>
          </>
        )}
      </div>
    </motion.div>
  );
}
