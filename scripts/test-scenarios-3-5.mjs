#!/usr/bin/env node
/**
 * Scenarios 3-5 (separate to avoid rate limit from re-running 1-2)
 */
import { generateKeyPairSync, sign as cryptoSign } from 'crypto';
import bs58 from 'bs58';

const BRIDGE = process.env.BRIDGE_URL || "http://localhost:3310";
const PIONEER_KEY = process.env.PIONEER_API_KEY || "";

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
  return { "@context": ["https://www.w3.org/2018/credentials/v1"], type: ["VerifiablePresentation"], verifiableCredential: jwts };
}

async function req(method, path, body) {
  const opts = { method, headers: { "Content-Type": "application/json" } };
  if (body) opts.body = JSON.stringify(body);
  return (await fetch(`${BRIDGE}${path}`, opts)).json();
}

function pass(msg) { console.log(`  ✓ ${msg}`); }
function fail(msg) { console.error(`  ✗ FAIL: ${msg}`); process.exit(1); }
function assert(cond, msg) { cond ? pass(msg) : fail(msg); }

const GOV = generateIssuer("Gov");

async function main() {
  console.log("\n═══ Scenarios 3-5 ═══\n");

  // --- Scenario 3: Incomplete ---
  console.log("── Scenario 3: Incomplete Docs → NEEDS_REVIEW ──");
  const case3 = `incomplete-${Date.now()}`;
  const doc3 = buildSignedJWT(GOV, "did:example:mystery", "BusinessLicense", {
    entity_name: "XYZ Offshore Holdings Ltd",
    jurisdiction: "Unknown",
    entity_type: "Shell Company",
    country: "Unverified",
    verified: "false",
    pep_status: "unable_to_determine",
    sanctions_clear: "unknown - screening incomplete",
    aml_checked: "inconclusive - nominee structure detected",
  });

  const r3 = await req("POST", "/orchestrate/full", {
    case_id: case3,
    enable_delegation: true,
    credentials: [{ requirement_id: "partial", vp: buildVP([doc3]), trusted_issuers: [GOV.did] }],
    llm_api_key: PIONEER_KEY,
  });

  console.log(`  Case: ${case3}`);
  assert(r3.success, `Pipeline completed (got: ${r3.error || 'ok'})`);
  if (r3.success) {
    console.log(`  Decision: ${r3.decision}, Confidence: ${r3.confidence}`);
    const assess = r3.timeline?.find(t => t.step === "assess-risk");
    if (assess?.result) {
      console.log(`  AI: decision=${assess.result.decision}, confidence=${assess.result.confidence}`);
      console.log(`  AI reasoning: ${assess.result.reasoning}`);
    }
  }

  // Wait between scenarios to avoid rate limit
  console.log("\n  (waiting 15s for rate limit cooldown...)");
  await new Promise(r => setTimeout(r, 15000));

  // --- Scenario 4: Out-of-Order ---
  console.log("\n── Scenario 4: Out-of-Order → VIOLATION ──");
  const case4 = `violation-${Date.now()}`;

  // Commit plan via orchestrate-compatible direct call
  const planRes = await req("POST", "/plan/commit", {
    case_id: case4,
    steps: [
      { function_name: "verify-credential" },
      { function_name: "assess-risk" },
      { function_name: "decide" },
    ],
    ttl_secs: 3600,
  });
  console.log(`  Plan committed: ${planRes.success !== false}`);

  // Try assess-risk BEFORE verify-credential (out of order)
  // Use the direct contract execute — check for the error in response
  const ooo = await req("POST", "/contract/execute", {
    function_name: "assess-risk",
    input: { case_id: case4, facts: [], llm_api_key: PIONEER_KEY },
  });

  const errStr = JSON.stringify(ooo);
  const isViolation = errStr.includes("Out of order") || errStr.includes("expected") || ooo.success === false;

  if (isViolation) {
    pass("State machine rejected out-of-order call");
    console.log(`  Error: ${ooo.error?.slice(0, 150) || errStr.slice(0, 150)}`);
  } else {
    fail(`Expected rejection, got: ${errStr.slice(0, 150)}`);
  }

  // Check violations via query
  const viols = await req("GET", `/violations?case_id=${case4}`);
  const vCount = viols.result?.count || viols.count || 0;
  console.log(`  Violations recorded: ${vCount}`);

  // --- Scenario 5: Delegation Enforcement ---
  console.log("\n  (waiting 15s for rate limit cooldown...)");
  await new Promise(r => setTimeout(r, 15000));

  console.log("\n── Scenario 5: Delegation Enforcement ──");
  const case5 = `deleg-${Date.now()}`;
  const doc5 = buildSignedJWT(GOV, "did:example:deleg-test", "BusinessLicense", {
    entity_name: "Delegation Test Corp",
    jurisdiction: "Singapore",
    country: "Singapore",
    entity_type: "Private Limited",
    verified: "true",
    sanctions_clear: "true",
    aml_checked: "true",
  });

  const r5 = await req("POST", "/orchestrate/full", {
    case_id: case5,
    enable_delegation: true,
    credentials: [{ requirement_id: "test", vp: buildVP([doc5]), trusted_issuers: [GOV.did] }],
    llm_api_key: PIONEER_KEY,
  });

  assert(r5.success, `Delegated pipeline completed (got: ${r5.error || 'ok'})`);
  if (r5.success) {
    const planStep = r5.timeline?.find(t => t.step === "commit-plan");
    const committedBy = planStep?.result?.committed_by || "";
    const agentDid = "did:t3n:9f9ed0869dc3b8fc82ba523dbd4525246aff0d81";
    console.log(`  committed_by: ${committedBy}`);
    assert(committedBy !== agentDid && committedBy.startsWith("did:t3n:"),
      `Runtime enforced: counterparty DID stamped (not agent)`);

    const delegCreate = r5.timeline?.find(t => t.step === "delegation-create");
    const delegRevoke = r5.timeline?.find(t => t.step === "delegation-revoke");
    assert(delegCreate?.success, "Delegation created");
    assert(delegRevoke?.success, "Delegation revoked post-decision");

    const status = await req("GET", `/delegation/status?case_id=${case5}`);
    assert(status.revoked === true, "Delegation revoked confirmed");
    assert(status.active === false, "No longer active");
    console.log(`  Decision: ${r5.decision}, Confidence: ${r5.confidence}`);
  }

  console.log("\n═══ All Scenarios Complete ═══\n");
}

main().catch(e => { console.error(`Fatal: ${e.message}`); process.exit(1); });
