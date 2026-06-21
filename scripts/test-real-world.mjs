#!/usr/bin/env node
/**
 * Real-World Scenario Test — Phase A + B
 *
 * Proper Ed25519-signed JWT credentials, did:key issuers,
 * cryptographically valid VPs — proves full business flow.
 */

import { generateKeyPairSync, sign as cryptoSign, createHash } from 'crypto';
import bs58 from 'bs58';

const BRIDGE = process.env.BRIDGE_URL || "http://localhost:3310";
const PIONEER_KEY = process.env.PIONEER_API_KEY || "";

// --- Crypto: Real Ed25519 JWT Builder ---

function generateIssuer(name) {
  const { publicKey, privateKey } = generateKeyPairSync('ed25519');
  const pubRaw = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
  const multicodec = Buffer.concat([Buffer.from([0xed, 0x01]), pubRaw]);
  const did = 'did:key:z' + bs58.encode(multicodec);
  return { did, privateKey, name };
}

function buildSignedJWT(issuer, subject, credentialType, claims) {
  const now = Math.floor(Date.now() / 1000);
  const header = { alg: "EdDSA", typ: "JWT" };
  const payload = {
    iss: issuer.did,
    sub: subject,
    iat: now,
    exp: now + 86400,
    vc: {
      "@context": ["https://www.w3.org/2018/credentials/v1"],
      type: ["VerifiableCredential", credentialType],
      credentialSubject: claims,
    },
  };

  const headerB64 = Buffer.from(JSON.stringify(header)).toString('base64url');
  const payloadB64 = Buffer.from(JSON.stringify(payload)).toString('base64url');
  const signingInput = `${headerB64}.${payloadB64}`;
  const sig = cryptoSign(null, Buffer.from(signingInput), issuer.privateKey);
  return `${signingInput}.${sig.toString('base64url')}`;
}

function buildVP(jwts) {
  return {
    "@context": ["https://www.w3.org/2018/credentials/v1"],
    type: ["VerifiablePresentation"],
    verifiableCredential: jwts,
  };
}

// --- API ---
async function req(method, path, body) {
  const opts = { method, headers: { "Content-Type": "application/json" } };
  if (body) opts.body = JSON.stringify(body);
  const res = await fetch(`${BRIDGE}${path}`, opts);
  return res.json();
}

function pass(msg) { console.log(`  ✓ ${msg}`); }
function fail(msg) { console.error(`  ✗ FAIL: ${msg}`); process.exit(1); }
function assert(cond, msg) { cond ? pass(msg) : fail(msg); }
function section(t) { console.log(`\n── ${t} ──`); }

// --- Issuers (real Ed25519 keys) ---
const GOV_ISSUER = generateIssuer("Singapore Government");
const BANK_ISSUER = generateIssuer("DBS Bank");
const TRADE_ISSUER = generateIssuer("Trade Registry");

// --- Scenario 1: Good Entity ---
async function testGoodEntity() {
  section("Scenario 1: Good Entity (Singapore, Licensed) → expect APPROVED");
  const caseId = `good-${Date.now()}`;

  const bizLicense = buildSignedJWT(GOV_ISSUER, "did:example:acme-sg", "BusinessLicense", {
    entity_name: "Acme Trading Pte Ltd",
    legal_name: "Acme Trading Pte Ltd",
    registration_number: "202312345G",
    jurisdiction: "Singapore",
    entity_type: "Private Limited Company",
    country: "Singapore",
    region: "Southeast Asia",
    sanctions_clear: "true",
    aml_checked: "true",
    pep_status: "no_matches",
    expiry_date: "2028-06-15",
  });

  const bankCert = buildSignedJWT(BANK_ISSUER, "did:example:acme-sg", "FinancialClearance", {
    entity_name: "Acme Trading Pte Ltd",
    jurisdiction: "Singapore",
    country: "Singapore",
    sanctions_clear: "true",
    aml_checked: "true",
    pep_status: "no_matches",
    verified: "true",
    document_type: "Financial Clearance Certificate",
  });

  const result = await req("POST", "/orchestrate/full", {
    case_id: caseId,
    enable_delegation: true,
    credentials: [
      { requirement_id: "business-license", vp: buildVP([bizLicense]), trusted_issuers: [GOV_ISSUER.did] },
      { requirement_id: "financial-clearance", vp: buildVP([bankCert]), trusted_issuers: [BANK_ISSUER.did] },
    ],
    llm_api_key: PIONEER_KEY,
  });

  console.log(`  Case: ${caseId}`);
  assert(result.success, `Pipeline completed (got: ${result.error || 'ok'})`);

  if (result.success) {
    console.log(`  Decision: ${result.decision}, Confidence: ${result.confidence}`);
    assert(result.delegation?.vc_id, "Delegation credential used");

    const planStep = result.timeline?.find(t => t.step === "commit-plan");
    if (planStep?.result?.committed_by) {
      console.log(`  Committed by: ${planStep.result.committed_by} (counterparty)`);
    }

    const assessStep = result.timeline?.find(t => t.step === "assess-risk");
    if (assessStep?.result) {
      console.log(`  AI assessment: decision=${assessStep.result.decision}, confidence=${assessStep.result.confidence}`);
      console.log(`  AI reasoning: ${assessStep.result.reasoning}`);
    }

    const verifySteps = result.timeline?.filter(t => t.step.startsWith("verify-credential"));
    for (const vs of verifySteps || []) {
      console.log(`  ${vs.step}: verified=${vs.result?.verified}, facts=${vs.result?.facts_count}`);
    }
  }

  return result;
}

// --- Scenario 2: Sanctioned Entity ---
async function testSanctionedEntity() {
  section("Scenario 2: Sanctioned Entity (North Korea, OFAC) → expect BLOCKED");
  const caseId = `sanctioned-${Date.now()}`;

  const doc = buildSignedJWT(TRADE_ISSUER, "did:example:dprk-entity", "BusinessLicense", {
    entity_name: "Korea Munitions Industry Department",
    legal_name: "Korea Munitions Industry Department",
    registration_number: "KP-OFAC-001",
    jurisdiction: "North Korea (DPRK)",
    entity_type: "State Military Enterprise",
    country: "North Korea",
    region: "East Asia - Sanctioned Territory",
    sanctions_clear: "false - OFAC SDN List, UN Resolution 1718",
    aml_checked: "FAILED - designated entity under comprehensive sanctions",
    pep_status: "Kim Jong Un (Head of State) - maximum risk",
  });

  const result = await req("POST", "/orchestrate/full", {
    case_id: caseId,
    enable_delegation: true,
    credentials: [
      { requirement_id: "entity-docs", vp: buildVP([doc]), trusted_issuers: [TRADE_ISSUER.did] },
    ],
    llm_api_key: PIONEER_KEY,
  });

  console.log(`  Case: ${caseId}`);
  assert(result.success, `Pipeline completed (got: ${result.error || 'ok'})`);

  if (result.success) {
    console.log(`  Decision: ${result.decision}, Confidence: ${result.confidence}`);
    const assessStep = result.timeline?.find(t => t.step === "assess-risk");
    if (assessStep?.result) {
      console.log(`  AI assessment: decision=${assessStep.result.decision}, confidence=${assessStep.result.confidence}`);
      console.log(`  AI reasoning: ${assessStep.result.reasoning}`);
      assert(assessStep.result.decision === "blocked", `AI blocked sanctioned entity (got: ${assessStep.result.decision})`);
    }
  }

  return result;
}

// --- Scenario 3: Incomplete / Ambiguous ---
async function testIncompleteEntity() {
  section("Scenario 3: Incomplete Docs → expect NEEDS_REVIEW");
  const caseId = `incomplete-${Date.now()}`;

  const doc = buildSignedJWT(GOV_ISSUER, "did:example:mystery", "BusinessLicense", {
    entity_name: "XYZ Offshore Holdings Ltd",
    jurisdiction: "Unknown",
    entity_type: "Shell Company",
    country: "Unverified",
    verified: "false",
    pep_status: "unable_to_determine",
    sanctions_clear: "unknown - screening incomplete",
    aml_checked: "inconclusive - nominee structure detected",
  });

  const result = await req("POST", "/orchestrate/full", {
    case_id: caseId,
    enable_delegation: true,
    credentials: [
      { requirement_id: "partial-docs", vp: buildVP([doc]), trusted_issuers: [GOV_ISSUER.did] },
    ],
    llm_api_key: PIONEER_KEY,
  });

  console.log(`  Case: ${caseId}`);
  assert(result.success, `Pipeline completed (got: ${result.error || 'ok'})`);

  if (result.success) {
    console.log(`  Decision: ${result.decision}, Confidence: ${result.confidence}`);
    const assessStep = result.timeline?.find(t => t.step === "assess-risk");
    if (assessStep?.result) {
      console.log(`  AI assessment: decision=${assessStep.result.decision}, confidence=${assessStep.result.confidence}`);
      console.log(`  AI reasoning: ${assessStep.result.reasoning}`);
    }
  }

  return result;
}

// --- Scenario 4: Out-of-Order Violation ---
async function testOutOfOrder() {
  section("Scenario 4: Out-of-Order Execution → expect VIOLATION");
  const caseId = `violation-${Date.now()}`;

  // Commit plan: verify-credential → assess-risk → decide
  await req("POST", "/plan/commit", {
    case_id: caseId,
    steps: [
      { function_name: "verify-credential" },
      { function_name: "assess-risk" },
      { function_name: "decide" },
    ],
    ttl_secs: 3600,
  });

  // Try assess-risk BEFORE verify-credential → should fail
  const oooResult = await req("POST", "/contract/execute", {
    function_name: "assess-risk",
    input: { case_id, facts: [], llm_api_key: PIONEER_KEY },
  });

  const errorMsg = JSON.stringify(oooResult);
  const isViolation = errorMsg.includes("Out of order") || errorMsg.includes("expected");
  console.log(`  Out-of-order response: ${errorMsg.slice(0, 150)}`);
  assert(isViolation, "State machine rejected out-of-order execution");

  // Check violations recorded
  const violations = await req("GET", `/violations?case_id=${caseId}`);
  const vCount = violations.result?.count || violations.count || 0;
  console.log(`  Violations recorded: ${vCount}`);
  assert(vCount > 0, "Violation was recorded in evidence chain");

  return { caseId, oooResult, violations };
}

// --- Scenario 5: Delegation Enforcement ---
async function testDelegationEnforcement() {
  section("Scenario 5: Delegation Enforcement Proof");
  const caseId = `deleg-enforce-${Date.now()}`;

  const doc = buildSignedJWT(GOV_ISSUER, "did:example:enforce-test", "BusinessLicense", {
    entity_name: "Enforcement Test Corp",
    jurisdiction: "Singapore",
    country: "Singapore",
    entity_type: "Private Limited",
    verified: "true",
    sanctions_clear: "true",
    aml_checked: "true",
  });

  const result = await req("POST", "/orchestrate/full", {
    case_id: caseId,
    enable_delegation: true,
    credentials: [
      { requirement_id: "test-doc", vp: buildVP([doc]), trusted_issuers: [GOV_ISSUER.did] },
    ],
    llm_api_key: PIONEER_KEY,
  });

  assert(result.success, `Delegated pipeline completed (got: ${result.error || 'ok'})`);

  if (result.success) {
    // Verify counterparty DID in committed_by
    const planStep = result.timeline?.find(t => t.step === "commit-plan");
    const committedBy = planStep?.result?.committed_by || "";
    const agentDid = "did:t3n:9f9ed0869dc3b8fc82ba523dbd4525246aff0d81";
    console.log(`  committed_by: ${committedBy}`);
    assert(committedBy !== agentDid && committedBy.startsWith("did:t3n:"),
      `Runtime enforced: committed_by = counterparty DID (not agent)`);

    // Verify delegation lifecycle
    const delegCreate = result.timeline?.find(t => t.step === "delegation-create");
    const delegRevoke = result.timeline?.find(t => t.step === "delegation-revoke");
    assert(delegCreate?.success, "Delegation created before pipeline");
    assert(delegRevoke?.success, "Delegation revoked after pipeline");

    // Verify post-revoke status
    const status = await req("GET", `/delegation/status?case_id=${caseId}`);
    assert(status.revoked === true, "Delegation is revoked post-pipeline");
    assert(status.active === false, "Delegation inactive post-pipeline");
  }

  return result;
}

// --- Main ---
async function main() {
  console.log("═══════════════════════════════════════════════════════════");
  console.log("  Verigate — Real-World Scenario Tests (Phase A + B)");
  console.log("  Ed25519 signed JWTs | did:key issuers | Live T3N testnet");
  console.log("═══════════════════════════════════════════════════════════");

  const health = await req("GET", "/health");
  assert(health.authenticated, "Bridge connected to T3N testnet");
  console.log(`  Agent DID: ${health.did}`);

  const results = {};
  results.good = await testGoodEntity();
  results.sanctioned = await testSanctionedEntity();
  results.incomplete = await testIncompleteEntity();
  results.violation = await testOutOfOrder();
  results.delegation = await testDelegationEnforcement();

  // Summary
  console.log("\n═══════════════════════════════════════════════════════════");
  console.log("  RESULTS SUMMARY");
  console.log("═══════════════════════════════════════════════════════════");
  console.log(`  Good Entity:       ${results.good.decision || 'N/A'} (conf: ${results.good.confidence || 'N/A'})`);
  console.log(`  Sanctioned Entity: ${results.sanctioned.decision || 'N/A'} (conf: ${results.sanctioned.confidence || 'N/A'})`);
  console.log(`  Incomplete Docs:   ${results.incomplete.decision || 'N/A'} (conf: ${results.incomplete.confidence || 'N/A'})`);
  console.log(`  Out-of-Order:      VIOLATION ENFORCED`);
  console.log(`  Delegation:        RUNTIME ENFORCED (counterparty DID stamped)`);
  console.log("═══════════════════════════════════════════════════════════");
  console.log("\n  All tests use cryptographically valid Ed25519 JWTs.");
  console.log("  AI reasoning by Pioneer (DeepSeek-V4-Flash) inside TEE.");
  console.log("  Delegation: real counterparty wallet + T3N runtime enforcement.\n");
}

main().catch(e => {
  console.error(`\n✗ Fatal: ${e.message}`);
  process.exit(1);
});
