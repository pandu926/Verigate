# Verigate

**Autonomous Counterparty Due Diligence Agent** — built on Terminal 3's TEE infrastructure.

An AI agent that conducts counterparty onboarding, credential verification, risk assessment, and trust decisions autonomously — with every action identity-bound, delegated by the data owner, and cryptographically auditable.

## What This Proves

| T3N Feature | How Verigate Uses It |
|-------------|---------------------|
| **TEE Contract Execution** | 11-function WASM state machine running inside T3N enclave |
| **Agent Auth SDK** | Scope enforcement, function-level grants, host allowlists |
| **Delegation Credentials** | `buildDelegationCredential` + EIP-191 signing — counterparty authorizes agent |
| **Delegated Execution** | `pii_did` mode — runtime stamps actor/subject/vc_id in audit trail |
| **AI in TEE** | Pioneer (DeepSeek-V4-Flash) called from inside enclave via `host:interfaces/http` |
| **Selective Disclosure** | `http-with-placeholders` — PII resolved by T3N host, contract only sees `{{profile.*}}` templates. Proven: `PlaceholderUnknown` error confirms host attempted resolution. |
| **State Machine** | Enforced step ordering with violation detection and recording |
| **Evidence Chain** | Per-step cryptographic hashes, immutable audit trail |

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│  Frontend (React + TypeScript)                          │
│  /evidence — Live pipeline viewer with animated timeline│
└──────────────────────────┬──────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────┐
│  Bridge (Node.js)                                       │
│  • Counterparty wallet authentication                   │
│  • Delegation credential lifecycle (create/sign/revoke) │
│  • Delegated contract execution with DelegationEnvelope │
│  • Ed25519 JWT credential builder                       │
└──────────────────────────┬──────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────┐
│  T3N TEE (WASM Contract — Rust)                         │
│  11 exported functions:                                  │
│  • commit-assessment-plan    • verify-credential        │
│  • assess-risk (AI)          • decide                   │
│  • set-compliance-policy     • execute-protected-action │
│  • get-plan-status           • get-evidence-chain       │
│  • get-violations            • advance-step             │
│  • append-to-evidence                                   │
└─────────────────────────────────────────────────────────┘
```

## Quick Start

### One-Command Setup

```bash
./setup.sh <YOUR_T3N_API_KEY>
```

That's it. Builds Docker images, starts all services, verifies T3N connection, and shows you what to do next.

### Prerequisites

- Docker & Docker Compose
- Node.js 22+
- T3N API key (testnet) — get one at https://terminal3.io

### Run with Docker

```bash
# Build fresh (no cache)
docker compose build --no-cache

# Start all services
docker compose up -d

# Verify
curl http://localhost:3309        # Frontend
curl http://localhost:3302/api/health  # Backend
curl http://localhost:3310/health      # T3N Bridge
```

### Run CLI Demo (fastest way to see it work)

```bash
cd t3n-client && npm ci
T3N_API_KEY=<your-key> node bridge.mjs &

# Run all scenarios
./verigate demo

# Or individual scenarios
./verigate demo --scenario good        # → APPROVED (0.95)
./verigate demo --scenario sanctioned  # → BLOCKED (1.0)
./verigate demo --scenario incomplete  # → NEEDS REVIEW
./verigate demo --scenario violation   # → VIOLATION ENFORCED
./verigate demo --scenario delegation  # → LIFECYCLE COMPLETE
```

### Run E2E Tests (Playwright)

```bash
cd e2e && npm ci && npx playwright install chromium
npx playwright test tests/evidence-chain.spec.ts --config=playwright.phase1.config.ts
```

## Live Demo

**URL:** http://verigate.rbexp.com

- `/evidence` — Evidence Chain viewer (run live pipeline scenarios)
- `/dashboard` — Reviewer dashboard
- `/portal/:caseId` — Counterparty submission portal

## Proven Results (Live T3N Testnet)

| Scenario | Decision | Confidence | AI Reasoning |
|----------|----------|-----------|--------------|
| Good entity (Singapore, licensed, AML clear) | **APPROVED** | 0.95 | "All checks passed: AML cleared, no PEP matches, sanctions clear" |
| Sanctioned entity (North Korea, OFAC SDN) | **BLOCKED** | 1.0 | "Designated North Korean state military enterprise under comprehensive sanctions" |
| Shell company (incomplete docs) | **NEEDS REVIEW** | 0.2 | "Inconclusive AML, nominee structure, unknown jurisdiction" |
| Out-of-order execution | **VIOLATION** | — | State machine rejected: "expected verify-credential, got assess-risk" |

## Delegation Flow (vs Self-Grant)

Most T3N implementations use self-grant (`agent-auth-update` by the agent itself). Verigate implements **proper W3C VC-based delegation**:

```
1. Counterparty wallet authenticates to T3N (separate identity)
2. Counterparty grants agent via agent-auth-update (on-chain tx)
3. buildDelegationCredential — scoped to case + functions + TTL
4. signCredential (EIP-191) — counterparty signs
5. executeDelegatedContract — pii_did mode, per-call agent signature
6. T3N runtime stamps: committed_by = counterparty DID (not agent)
7. Auto-revoke after decision — agent access withdrawn
```

**Runtime enforcement proven:** Without grant, T3N returns `403 Forbidden: not permitted to act on behalf of`.

## Contract Functions (11 total)

| Function | Purpose | T3N Feature Used |
|----------|---------|-----------------|
| `commit-assessment-plan` | Lock step ordering | KV store (plan:meta + plan:cursor) |
| `verify-credential` | Ed25519/P-256 JWT verification | did:key resolution, verify_step |
| `assess-risk` | AI risk assessment | `host:interfaces/http` (Pioneer AI) |
| `decide` | Final decision based on AI confidence | Evidence chain reads |
| `set-compliance-policy` | Configure thresholds | KV store |
| `execute-protected-action` | Gate post-decision actions | Decision check + violation recording |
| `get-plan-status` | Query plan state | KV read |
| `get-evidence-chain` | Read full evidence | Per-step KV reads |
| `get-violations` | Read violations | KV scan |
| `advance-step` | Manual step advance | Cursor write |
| `append-to-evidence` | Manual evidence entry | KV write |

## Technology Stack

| Layer | Technology |
|-------|-----------|
| Contract | Rust → wasm32-wasip2, deployed on T3N TEE |
| Bridge | Node.js + @terminal3/t3n-sdk v3.9.0 |
| Backend | Rust + Axum + SQLx + PostgreSQL |
| Frontend | React 19 + TypeScript + Vite + Framer Motion |
| AI | Pioneer (DeepSeek-V4-Flash) called from TEE enclave |
| Credentials | Ed25519 signed JWTs, did:key, W3C VC Data Model |
| Delegation | buildDelegationCredential + EIP-191 + signAgentInvocation |

## Project Structure

```
terminal/
├── t3n-contract/     # Rust WASM contract (11 functions)
│   └── src/          # lib.rs, plan.rs, policy.rs, state.rs, crypto.rs, claims.rs
├── t3n-client/       # Node.js bridge to T3N
│   └── bridge.mjs   # SDK integration, delegation, scenarios
├── frontend/         # React Evidence Chain viewer
│   └── src/pages/EvidenceChain.tsx
├── backend/          # Rust API server
├── e2e/              # Playwright E2E tests
├── scripts/          # CLI demo, test scripts
│   └── verigate-cli.mjs
└── verigate          # CLI entry point
```

## vs Competition

| Feature | Verigate | Typical Submission |
|---------|----------|-------------------|
| Contract functions | 11 | 3-4 |
| AI in TEE | Yes (Pioneer) | No |
| Delegation credentials | W3C VC + EIP-191 | Self-grant only |
| Runtime enforcement | Proven (403 without grant) | Not tested |
| Credential verification | Ed25519 cryptographic | Mock/none |
| State machine | Enforced ordering + violations | Basic |
| Evidence chain | Per-step hashes | Minimal |
| Decision logic | AI confidence → threshold | Hardcoded |
