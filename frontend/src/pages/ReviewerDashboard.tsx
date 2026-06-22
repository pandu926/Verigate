import { useState, useMemo } from 'react';
import { Link } from 'react-router-dom';
import { motion, AnimatePresence } from 'framer-motion';
import { useCases } from '@/hooks/useCases';
import { useCreateCase } from '@/hooks/useCreateCase';
import { AnimatedCounter } from '@/components/ui/AnimatedCounter';
import { ShieldIcon, ClockIcon, CheckIcon, AlertIcon, ChevronRightIcon, GlobeIcon, DocumentIcon, SendIcon } from '@/components/ui/Icons';
import type { DemoCase } from '@/types/demo';
import './ReviewerDashboard.css';

const STATUS_STEPS = ['created', 'collecting', 'verifying', 'assessing', 'review', 'approved'] as const;

function getStatusIndex(status: string): number {
  const idx = STATUS_STEPS.indexOf(status as typeof STATUS_STEPS[number]);
  return idx >= 0 ? idx : 0;
}

function getStatusColor(status: string): string {
  switch (status) {
    case 'approved': return 'var(--color-success, #34d399)';
    case 'blocked': return 'var(--color-danger, #f87171)';
    case 'review': return 'var(--color-warning, #fbbf24)';
    default: return '#5eead4';
  }
}

export function ReviewerDashboard() {
  const { data: cases, isLoading, isError } = useCases();
  const [search, setSearch] = useState('');
  const [statusFilter, setStatusFilter] = useState('all');
  const [showCreate, setShowCreate] = useState(false);

  const stats = useMemo(() => {
    if (!cases) return { total: 0, collecting: 0, approved: 0, blocked: 0 };
    return {
      total: cases.length,
      collecting: cases.filter(c => ['created', 'collecting', 'verifying', 'assessing'].includes(c.status)).length,
      approved: cases.filter(c => c.status === 'approved').length,
      blocked: cases.filter(c => c.status === 'blocked').length,
    };
  }, [cases]);

  const filteredCases = useMemo(() => {
    if (!cases) return [];
    let result = cases;
    if (statusFilter !== 'all') {
      if (statusFilter === 'active') {
        result = result.filter(c => !['approved', 'blocked'].includes(c.status));
      } else {
        result = result.filter(c => c.status === statusFilter);
      }
    }
    if (search) {
      const q = search.toLowerCase();
      result = result.filter(c => c.entity_name?.toLowerCase().includes(q) || c.jurisdiction?.toLowerCase().includes(q));
    }
    return result;
  }, [cases, statusFilter, search]);

  if (isLoading) {
    return (
      <div className="dash">
        <div className="dash__loading">
          <motion.div animate={{ opacity: [0.3, 1, 0.3] }} transition={{ duration: 1.5, repeat: Infinity }}>
            Loading cases...
          </motion.div>
        </div>
      </div>
    );
  }

  if (isError) {
    return (
      <div className="dash">
        <div className="dash__error">Unable to load cases. Check backend connection.</div>
      </div>
    );
  }

  return (
    <div className="dash">
      <header className="dash__header">
        <div>
          <h1 className="dash__title">Case Dashboard</h1>
          <p className="dash__subtitle">Counterparty verification cases</p>
        </div>
        <button className="dash__create-btn" onClick={() => setShowCreate(true)}>
          <SendIcon size={16} />
          New Case
        </button>
      </header>

      {/* Create Case Modal */}
      <AnimatePresence>
        {showCreate && <CreateCaseModal onClose={() => setShowCreate(false)} />}
      </AnimatePresence>

      {/* Stats */}
      <div className="dash__stats">
        <motion.div className="dash__stat" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.1 }}>
          <div className="dash__stat-value"><AnimatedCounter value={stats.total} /></div>
          <div className="dash__stat-label">Total Cases</div>
        </motion.div>
        <motion.div className="dash__stat dash__stat--active" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.2 }}>
          <div className="dash__stat-value"><AnimatedCounter value={stats.collecting} /></div>
          <div className="dash__stat-label">In Progress</div>
        </motion.div>
        <motion.div className="dash__stat dash__stat--success" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.3 }}>
          <div className="dash__stat-value"><AnimatedCounter value={stats.approved} /></div>
          <div className="dash__stat-label">Approved</div>
        </motion.div>
        <motion.div className="dash__stat dash__stat--danger" initial={{ opacity: 0, y: 20 }} animate={{ opacity: 1, y: 0 }} transition={{ delay: 0.4 }}>
          <div className="dash__stat-value"><AnimatedCounter value={stats.blocked} /></div>
          <div className="dash__stat-label">Blocked</div>
        </motion.div>
      </div>

      {/* Filters */}
      <div className="dash__filters">
        <input
          className="dash__search"
          type="text"
          placeholder="Search by name or jurisdiction..."
          value={search}
          onChange={e => setSearch(e.target.value)}
        />
        <select className="dash__filter-select" value={statusFilter} onChange={e => setStatusFilter(e.target.value)}>
          <option value="all">All Status</option>
          <option value="active">In Progress</option>
          <option value="approved">Approved</option>
          <option value="blocked">Blocked</option>
          <option value="review">Under Review</option>
        </select>
      </div>

      {/* Case List */}
      <div className="dash__list">
        {filteredCases.length === 0 ? (
          <div className="dash__empty">
            No cases yet.
            <button className="dash__empty-btn" onClick={() => setShowCreate(true)}>Create your first case</button>
          </div>
        ) : (
          filteredCases.map((c: DemoCase, i: number) => (
            <motion.div
              key={c.id}
              className="dash__case"
              initial={{ opacity: 0, y: 16 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ delay: i * 0.05, duration: 0.3 }}
            >
              <div className="dash__case-main">
                <div className="dash__case-icon">
                  <ShieldIcon size={20} />
                </div>
                <div className="dash__case-info">
                  <h3 className="dash__case-name">{c.entity_name || 'Unknown Entity'}</h3>
                  <div className="dash__case-meta">
                    <span className="dash__case-tag"><GlobeIcon size={12} /> {c.jurisdiction || 'Global'}</span>
                    <span className="dash__case-tag"><DocumentIcon size={12} /> {c.workflow_type || 'onboarding'}</span>
                  </div>
                </div>
                <div className="dash__case-status">
                  <span className="dash__case-badge" style={{ color: getStatusColor(c.status) }}>
                    {c.status === 'approved' && <CheckIcon size={12} />}
                    {c.status === 'blocked' && <AlertIcon size={12} />}
                    {!['approved', 'blocked'].includes(c.status) && <ClockIcon size={12} />}
                    {c.status}
                  </span>
                </div>
              </div>

              {/* Progress stepper */}
              <div className="dash__case-progress">
                {STATUS_STEPS.map((step, idx) => (
                  <div key={step} className={`dash__step ${idx <= getStatusIndex(c.status) ? 'dash__step--done' : ''} ${idx === getStatusIndex(c.status) ? 'dash__step--current' : ''}`}>
                    <div className="dash__step-dot" />
                    {idx < STATUS_STEPS.length - 1 && <div className="dash__step-line" />}
                  </div>
                ))}
              </div>

              {/* Actions */}
              <div className="dash__case-actions">
                <Link to={`/case/${c.id}`} className="dash__action dash__action--primary">
                  Review Case <ChevronRightIcon size={14} />
                </Link>
                <Link to={`/privacy/${c.id}`} className="dash__action dash__action--ghost">
                  Privacy View
                </Link>
              </div>
            </motion.div>
          ))
        )}
      </div>
    </div>
  );
}

function CreateCaseModal({ onClose }: { onClose: () => void }) {
  const [form, setForm] = useState({
    entity_name: '',
    entity_type: 'Corporation',
    workflow_type: 'Onboarding',
    jurisdiction: 'Singapore',
    relationship_goal: 'Investment Partnership',
  });
  const [error, setError] = useState('');

  const { mutate, isPending } = useCreateCase({
    onSuccess: () => onClose(),
    onError: (msg) => setError(msg),
  });

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (!form.entity_name.trim()) {
      setError('Entity name is required');
      return;
    }
    setError('');
    mutate(form);
  };

  return (
    <motion.div
      className="modal-overlay"
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      onClick={onClose}
    >
      <motion.div
        className="modal"
        initial={{ opacity: 0, scale: 0.95, y: 20 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.95, y: 20 }}
        onClick={e => e.stopPropagation()}
      >
        <h2 className="modal__title">Create New Case</h2>
        <p className="modal__desc">Start a counterparty verification process</p>

        <form className="modal__form" onSubmit={handleSubmit}>
          <label className="modal__field">
            <span>Entity Name *</span>
            <input
              type="text"
              value={form.entity_name}
              onChange={e => setForm(f => ({ ...f, entity_name: e.target.value }))}
              placeholder="e.g. Meridian Capital Partners"
              autoFocus
            />
          </label>

          <label className="modal__field">
            <span>Entity Type</span>
            <select value={form.entity_type} onChange={e => setForm(f => ({ ...f, entity_type: e.target.value }))}>
              <option value="Corporation">Corporation</option>
              <option value="Individual">Individual</option>
              <option value="Fund">Fund</option>
              <option value="Trust">Trust</option>
              <option value="Dao">DAO</option>
              <option value="Government">Government</option>
            </select>
          </label>

          <label className="modal__field">
            <span>Workflow Type</span>
            <select value={form.workflow_type} onChange={e => setForm(f => ({ ...f, workflow_type: e.target.value }))}>
              <option value="Onboarding">Onboarding</option>
              <option value="DueDiligence">Due Diligence</option>
              <option value="Compliance">Compliance</option>
              <option value="Revalidation">Revalidation</option>
            </select>
          </label>

          <label className="modal__field">
            <span>Jurisdiction</span>
            <input
              type="text"
              value={form.jurisdiction}
              onChange={e => setForm(f => ({ ...f, jurisdiction: e.target.value }))}
              placeholder="e.g. Singapore, US, EU"
            />
          </label>

          <label className="modal__field">
            <span>Relationship Goal</span>
            <input
              type="text"
              value={form.relationship_goal}
              onChange={e => setForm(f => ({ ...f, relationship_goal: e.target.value }))}
              placeholder="e.g. Investment Partnership"
            />
          </label>

          {error && <div className="modal__error">{error}</div>}

          <div className="modal__actions">
            <button type="button" className="modal__btn modal__btn--ghost" onClick={onClose}>Cancel</button>
            <button type="submit" className="modal__btn modal__btn--primary" disabled={isPending}>
              {isPending ? 'Creating...' : 'Create Case'}
            </button>
          </div>
        </form>
      </motion.div>
    </motion.div>
  );
}
