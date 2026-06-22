# Verigate Demo Video Script — 3 Minutes

## Overview
- Language: Indonesian + English (technical terms in English)
- Tone: Confident, technical but accessible
- Goal: Judges understand the problem, see real T3N integration depth, and are impressed by the completeness
- Format: Narasi kamu + animated slides + live terminal output

---

## [0:00 - 0:20] HOOK — The Problem

**Visual:** Dark screen, text overlay animasi

**Narasi (ID):**
"Counterparty onboarding hari ini itu nightmare. Data sensitif — KTP, dokumen perusahaan, laporan keuangan — dikirim ke puluhan sistem berbeda. Tidak ada proof siapa yang akses apa. Tidak ada cara revoke akses setelah keputusan dibuat. Bagaimana kalau ada AI agent yang bisa onboard counterparty — tapi data sensitif TIDAK PERNAH keluar dari trusted enclave?"

**Text overlay:**
"The Problem: Sensitive data flows everywhere. No boundaries. No revocation."

---

## [0:20 - 0:50] SOLUTION — What is Verigate

**Visual:** Architecture diagram animasi

**Narasi (ID + EN):**
"Verigate adalah autonomous counterparty agent yang berjalan di dalam T3N TEE — Trusted Execution Environment. Bukan satu fungsi — tapi 11 fungsi kontrak yang saling terkoneksi membentuk state machine."

"The key innovation: the counterparty — the DATA OWNER — cryptographically delegates authority to the agent. The agent can ONLY act within the scope the data owner defined. And after the decision is made? Delegation is automatically revoked."

"Ini bukan self-grant seperti implementasi lain. Ini PROPER W3C Verifiable Credential delegation dengan EIP-191 signature."

**Text overlay (berurutan):**
- "11 TEE Contract Functions"
- "W3C Delegation Credentials"
- "AI Reasoning Inside Enclave"

---

## [0:50 - 1:20] DELEGATION — The Core Innovation

**Visual:** Terminal showing delegation lifecycle

**Narasi (EN — technical):**
"Let me show you the delegation flow that makes Verigate different from every other submission."

"Step one — a separate counterparty wallet authenticates to T3N. This is a REAL second identity, not a mock."

"Step two — the counterparty grants the agent permission to act on their behalf. This is an on-chain transaction."

"Step three — buildDelegationCredential from the T3N SDK. Scoped to specific functions, specific case, with a TTL. The counterparty signs it with EIP-191."

"Step four — every contract call carries a DelegationEnvelope. The T3N runtime validates the grant chain. Without it? 403 Forbidden."

"Step five — after the decision, delegation is automatically revoked. Agent access is gone."

**Text overlay:**
- "Counterparty wallet: did:t3n:bec292d1..."
- "Agent wallet: did:t3n:30c0e33d..."
- "Runtime enforced: 403 without grant"
- "Auto-revoke after decision"

---

## [1:20 - 2:00] LIVE DEMO — Scenarios

**Visual:** Terminal running ./verigate demo scenarios

**Narasi (ID):**
"Mari kita lihat live. Semua ini berjalan di T3N testnet — bukan mock."

[Show: Good Entity]
"Entity pertama — Acme Trading, Singapore. Licensed, AML clear, no PEP matches. AI di dalam TEE menganalisis 19 facts dari 2 Ed25519-signed credentials."

"Hasil: APPROVED, confidence 0.95. AI reasoning: 'All checks passed, sanctions clear, registered in low-risk Singapore.'"

[Show: Sanctioned Entity]
"Entity kedua — Korea Munitions Industry Department. North Korea, OFAC SDN List."

"AI langsung block. Confidence 1.0. Reasoning: 'Designated North Korean state military enterprise under comprehensive sanctions.' Tidak ada keraguan."

[Show: Violation]
"Dan kalau agent coba skip step? State machine REJECT. 'Expected verify-credential, got assess-risk.' Violation recorded permanently."

**Text overlay:**
- "Good Entity → APPROVED (0.95)"
- "Sanctioned → BLOCKED (1.0)"
- "Violation → ENFORCED"

---

## [2:00 - 2:30] SELECTIVE DISCLOSURE + EVIDENCE CHAIN

**Visual:** Split — left: contract code showing {{profile.*}}, right: T3N response

**Narasi (EN):**
"Two more critical features that prove the depth of integration."

"Selective disclosure — the contract sends template placeholders like {{profile.first_name}}, {{profile.company_name}}. The T3N HOST resolves these to actual PII OUTSIDE the contract's memory. The contract never sees raw data. We proved this live — T3N returned PlaceholderUnknown, confirming the host attempted resolution."

"Evidence chain — every step produces a cryptographic hash. verify-credential, assess-risk, decide — each writes an immutable evidence entry. The entire chain is auditable, tamper-proof, and tied to the delegation credential."

**Text overlay:**
- "Contract sees: {{profile.email}} — never raw PII"
- "Evidence: per-step SHA-256 hashes"

---

## [2:30 - 2:50] SDK INTEGRATION DEPTH

**Visual:** Feature grid showing all SDK functions used

**Narasi (EN):**
"In total, Verigate uses 12+ functions from the T3N Agent Auth SDK:"

"buildDelegationCredential, signCredential, signAgentInvocation, revokeDelegation — the full delegation stack."

"pii_did delegated execution mode — T3N runtime stamps actor, subject, and vc_id in the audit trail."

"http-with-placeholders for selective disclosure. host:interfaces/http for AI calls from inside the enclave."

"agent-auth-update for scope enforcement. KV store for state machine. All 11 contract functions deployed and proven."

"This is not surface-level integration. This is the ENTIRE T3N platform being showcased."

**Text overlay:**
- "12+ SDK functions"
- "Not surface-level — the entire platform"

---

## [2:50 - 3:00] CLOSING

**Visual:** Landing page + URLs

**Narasi (ID):**
"Verigate. Counterparty onboarding yang autonomous, tapi bounded. Setiap aksi identity-bound. Setiap keputusan cryptographically auditable. Dan data sensitif — tidak pernah keluar dari TEE."

**Text overlay:**
"github.com/pandu926/Verigate"
"verigate.rbexp.com"
"Built with Terminal 3 Agent Auth SDK"

---

## Production Notes

- Record at 1080p, 30fps minimum
- Video HTML auto-plays slides (open demo/video.html in browser, screen record)
- Add narasi sebagai voiceover (record terpisah, overlay)
- Subtle dark ambient background music
- Total duration: ~3 minutes
- Show real terminal output / live app — bukan mockup
- Bisa screen record langsung dari verigate.rbexp.com/evidence untuk live demo portion
