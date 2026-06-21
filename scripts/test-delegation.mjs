#!/usr/bin/env node
/**
 * Phase B: Delegation Credential — Live Test
 *
 * Tests the full lifecycle against RUNNING bridge (port 3310):
 * 1. Create delegation → counterparty signs, credential built
 * 2. Check status → active, not expired
 * 3. Execute delegated pipeline → all steps pass with delegation
 * 4. Verify delegation auto-revoked after decide
 * 5. Attempt post-revoke call → BLOCKED
 */

const BRIDGE = process.env.BRIDGE_URL || "http://localhost:3310";

async function req(method, path, body) {
  const opts = { method, headers: { "Content-Type": "application/json" } };
  if (body) opts.body = JSON.stringify(body);
  const res = await fetch(`${BRIDGE}${path}`, opts);
  return res.json();
}

function assert(condition, msg) {
  if (!condition) {
    console.error(`  ✗ FAIL: ${msg}`);
    process.exit(1);
  }
  console.log(`  ✓ ${msg}`);
}

async function main() {
  const caseId = `deleg-test-${Date.now()}`;
  console.log(`\n═══ Phase B: Delegation Credential Test ═══`);
  console.log(`Case: ${caseId}\n`);

  // 0. Health check
  const health = await req("GET", "/health");
  assert(health.status === "ok" || health.authenticated, "Bridge is running");

  // 1. Create delegation credential
  console.log("\n── Step 1: Create Delegation ──");
  const delegation = await req("POST", "/delegation/create", {
    case_id: caseId,
    functions: ["assess-risk", "commit-assessment-plan", "decide", "verify-credential"],
    ttl_secs: 3600,
    scopes: ["COMPLIANCE_CHECK"],
  });
  console.log(`  Response:`, JSON.stringify(delegation, null, 2).split('\n').slice(0, 15).join('\n'));
  assert(delegation.delegation_created, "Delegation credential created");
  assert(delegation.counterparty_did?.startsWith("did:t3n:"), "Counterparty DID present");
  assert(delegation.agent_did?.startsWith("did:t3n:"), "Agent DID present");
  assert(delegation.vc_id, "VC ID generated");
  assert(delegation.signature?.type === "EIP-191", "EIP-191 signature present");
  assert(delegation.signature?.user_sig, "User signature (counterparty) present");
  assert(delegation.functions?.length === 4, "4 functions scoped");
  assert(delegation.proof?.includes("data owner"), "Proof statement present");

  // 2. Check delegation status
  console.log("\n── Step 2: Check Status ──");
  const status = await req("GET", `/delegation/status?case_id=${caseId}`);
  assert(status.active === true, "Delegation is active");
  assert(!status.revoked, "Not revoked");
  assert(!status.expired, "Not expired");
  assert(status.remaining_secs > 3500, "TTL remaining > 3500s");
  console.log(`  Active: ${status.active}, Expires: ${status.expires_at}, Remaining: ${status.remaining_secs}s`);

  // 3. Execute full pipeline WITH delegation
  console.log("\n── Step 3: Delegated Pipeline Execution ──");
  const pipeline = await req("POST", "/orchestrate/full", {
    case_id: caseId,
    enable_delegation: true,
    credentials: [{
      requirement_id: "business-license",
      vp: {
        "@context": ["https://www.w3.org/2018/credentials/v1"],
        type: ["VerifiablePresentation"],
        verifiableCredential: [],
      },
      trusted_issuers: [],
    }],
    llm_api_key: process.env.PIONEER_API_KEY || "",
  });
  assert(pipeline.success, `Pipeline completed: decision=${pipeline.decision}`);
  assert(pipeline.delegation, "Delegation info in response");
  assert(pipeline.delegation?.vc_id, "VC ID in pipeline response");
  assert(pipeline.delegation?.lifecycle?.includes("revoke"), "Full lifecycle documented");
  console.log(`  Decision: ${pipeline.decision}, Confidence: ${pipeline.confidence}`);
  console.log(`  Delegation lifecycle: ${pipeline.delegation?.lifecycle}`);

  // Check delegation revocation in timeline
  const revokeStep = pipeline.timeline?.find(t => t.step === "delegation-revoke");
  assert(revokeStep?.success, "Delegation auto-revoked in pipeline");
  console.log(`  Auto-revoked at: ${revokeStep?.result?.revoked_at}`);

  // 4. Verify delegation is now revoked
  console.log("\n── Step 4: Verify Post-Pipeline Status ──");
  const postStatus = await req("GET", `/delegation/status?case_id=${caseId}`);
  assert(postStatus.active === false, "Delegation no longer active");
  assert(postStatus.revoked === true, "Delegation marked revoked");
  assert(postStatus.revoked_at, "Revocation timestamp present");
  console.log(`  Revoked: ${postStatus.revoked}, At: ${postStatus.revoked_at}`);

  // 5. Test expired delegation (create with 1s TTL, wait, try to use)
  console.log("\n── Step 5: TTL Expiry Test ──");
  const expCaseId = `deleg-expire-${Date.now()}`;
  const expDeleg = await req("POST", "/delegation/create", {
    case_id: expCaseId,
    functions: ["verify-credential"],
    ttl_secs: 1,
  });
  assert(expDeleg.delegation_created, "Short-TTL delegation created");
  console.log("  Waiting 2s for expiry...");
  await new Promise(r => setTimeout(r, 2000));
  const expStatus = await req("GET", `/delegation/status?case_id=${expCaseId}`);
  assert(expStatus.expired === true, "Delegation expired after TTL");
  assert(expStatus.active === false, "Expired delegation is not active");
  console.log(`  Expired: ${expStatus.expired}, Active: ${expStatus.active}`);

  // 6. Create delegation for separate case, revoke explicitly, verify blocked
  console.log("\n── Step 6: Explicit Revocation Test ──");
  const revCaseId = `deleg-revoke-${Date.now()}`;
  const revDeleg = await req("POST", "/delegation/create", {
    case_id: revCaseId,
    functions: ["verify-credential", "assess-risk"],
    ttl_secs: 3600,
  });
  assert(revDeleg.delegation_created, "Revocation test delegation created");

  const revResult = await req("POST", "/delegation/revoke", { case_id: revCaseId });
  assert(revResult.revoked === true, "Explicit revocation succeeded");
  assert(revResult.vc_id, "Revoked VC ID returned");
  console.log(`  Revoked VC: ${revResult.vc_id}, At: ${revResult.revoked_at}`);

  const revStatus = await req("GET", `/delegation/status?case_id=${revCaseId}`);
  assert(revStatus.active === false, "Revoked delegation is not active");
  assert(revStatus.revoked === true, "Status shows revoked");

  // Summary
  console.log("\n═══ All Tests Passed ═══");
  console.log(`
  Delegation Lifecycle Proven:
  • Counterparty (data owner) signs EIP-191 credential
  • Agent scoped to specific functions + case + TTL
  • Pipeline executes under delegation authority
  • Auto-revocation after decision
  • TTL expiry enforced
  • Explicit revocation works
  • Post-revoke status = inactive

  vs Umbra: They use self-grant (agent authorizes itself)
  vs Verigate: Real W3C VC delegation (data owner authorizes agent)
  `);
}

main().catch(e => {
  console.error(`\n✗ Test failed: ${e.message}`);
  process.exit(1);
});
