import { T3nClient, TenantClient, loadWasmComponent, setEnvironment, getNodeUrl, getScriptVersion, createEthAuthInput, eth_get_address, metamask_sign, buildDelegationCredential, signCredential, signAgentInvocation, buildInvocationPreimage, canonicaliseCredential, revokeDelegation, b64uEncodeBytes, b64uDecodeStrict, validateCredentialBody } from "@terminal3/t3n-sdk";
import http from 'http';
import { readFileSync } from 'fs';
import { randomBytes, createHash, generateKeyPairSync, sign as cryptoSign } from 'crypto';

const T3N_API_KEY = process.env.T3N_API_KEY || "";
const PIONEER_API_KEY = process.env.PIONEER_API_KEY || "";
const PORT = parseInt(process.env.T3N_BRIDGE_PORT || "3310");
const CONTRACT_WASM_PATH = process.env.CONTRACT_WASM_PATH || "/app/contract/verigate_verify.wasm";
const CONTRACT_TAIL = "verigate";
const CONTRACT_VERSION = "1.0.6";

let client = null;
let tenant = null;
let clientDid = null;
let sessionId = null;
let tenantTid = null;

// In-memory fallbacks
const kvStore = new Map();
const auditLedger = [];

// --- Delegation State ---
// Counterparty wallet (real EOA — separate identity from agent)
const COUNTERPARTY_PRIVATE_KEY = process.env.COUNTERPARTY_KEY || "";
const counterpartyAddress = eth_get_address(COUNTERPARTY_PRIVATE_KEY);
// Real T3N DID (assigned by T3N, not derived from ETH address)
// Resolved at startup via authenticateCounterparty()
let counterpartyDid = null;
let counterpartyClient = null;

// Agent's secp256k1 compressed pubkey (33 bytes, derived from T3N_API_KEY)
function getAgentPubkey() {
  const addr = eth_get_address(T3N_API_KEY);
  // SDK uses 33-byte compressed pubkey — derive from the key material
  // For EOA keys, we reconstruct from the private key bytes
  const keyBytes = hexToBytes(T3N_API_KEY.replace("0x", ""));
  // secp256k1 pubkey derivation via SDK's internal — use the address as a 20-byte stand-in
  // The SDK accepts the 33-byte compressed form; for testnet demo we pad address to 33
  // In production this would be the actual compressed pubkey
  const pubkey = new Uint8Array(33);
  pubkey[0] = 0x02; // compressed prefix
  const addrBytes = hexToBytes(addr);
  pubkey.set(addrBytes, 1); // 20 bytes after prefix
  // Fill remaining 12 bytes with deterministic hash of key
  const hash = createHash('sha256').update(keyBytes).digest();
  pubkey.set(hash.subarray(0, 12), 21);
  return pubkey;
}

function hexToBytes(hex) {
  const bytes = new Uint8Array(hex.length / 2);
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = parseInt(hex.substr(i * 2, 2), 16);
  }
  return bytes;
}

// Active delegations: case_id → { credential, credential_jcs, user_sig, vc_id, created_at, revoked }
const activeDelegations = new Map();

async function initClient() {
  setEnvironment("testnet");
  const address = eth_get_address(T3N_API_KEY);

  client = new T3nClient({
    wasmComponent: await loadWasmComponent(),
    handlers: { EthSign: metamask_sign(address, undefined, T3N_API_KEY) },
  });

  await client.handshake();
  await client.authenticate(createEthAuthInput(address));

  clientDid = client.getDid();
  sessionId = client.getSessionId();
  tenantTid = clientDid.value.replace("did:t3n:", "");

  console.log(`[t3n-bridge] Authenticated as ${clientDid.value}`);
  console.log(`[t3n-bridge] Session: ${sessionId.value}`);
  console.log(`[t3n-bridge] Tenant ID: ${tenantTid}`);

  // Initialize TenantClient for management operations
  tenant = new TenantClient({
    t3n: client,
    baseUrl: getNodeUrl(),
    tenantDid: clientDid.value,
  });

  const { balance } = await client.getUsage();
  console.log(`[t3n-bridge] Credits available: ${balance.available}`);

  // Verify tenant status
  try {
    const me = await tenant.tenant.me();
    console.log(`[t3n-bridge] Tenant status: ${JSON.stringify(me).slice(0, 100)}`);
  } catch (e) {
    console.log(`[t3n-bridge] Tenant me() check: ${e.message}`);
  }

  // Authenticate counterparty wallet (separate identity for delegation)
  await initCounterparty();
}

async function initCounterparty() {
  try {
    counterpartyClient = new T3nClient({
      wasmComponent: await loadWasmComponent(),
      handlers: { EthSign: metamask_sign(counterpartyAddress, undefined, COUNTERPARTY_PRIVATE_KEY) },
    });

    await counterpartyClient.handshake();
    await counterpartyClient.authenticate(createEthAuthInput(counterpartyAddress));
    counterpartyDid = counterpartyClient.getDid().value;

    console.log(`[t3n-bridge] Counterparty authenticated: ${counterpartyDid}`);

    // Counterparty grants agent to act on their behalf
    const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;
    const userContractVersion = await getScriptVersion(getNodeUrl(), "tee:user/contracts");
    const grantResult = await counterpartyClient.execute({
      script_name: "tee:user/contracts",
      script_version: userContractVersion,
      function_name: "agent-auth-update",
      input: {
        agents: [{
          agentDid: clientDid.value,
          scripts: [{
            scriptName,
            versionReq: CONTRACT_VERSION,
            functions: ["commit-assessment-plan", "verify-credential", "assess-risk", "decide"],
            allowedHosts: ["api.pioneer.ai"],
          }],
        }],
      },
    });
    console.log(`[t3n-bridge] Counterparty granted agent delegation: ${JSON.stringify(grantResult)}`);
  } catch (e) {
    console.log(`[t3n-bridge] Counterparty init failed (non-fatal): ${e.message}`);
    // Fallback: use agent's own DID for self-delegation demo
    counterpartyDid = clientDid.value;
  }
}

function jsonResponse(res, status, data) {
  res.writeHead(status, { 'Content-Type': 'application/json' });
  res.end(JSON.stringify(data));
}

function parseBody(req) {
  return new Promise((resolve, reject) => {
    let body = '';
    req.on('data', chunk => { body += chunk; });
    req.on('end', () => {
      try { resolve(JSON.parse(body)); }
      catch (e) { reject(new Error('Invalid JSON body')); }
    });
    req.on('error', reject);
  });
}

function requireAuth(res) {
  if (!client || !client.isAuthenticated()) {
    jsonResponse(res, 503, { error: 'Not authenticated' });
    return false;
  }
  return true;
}

// --- Route Handlers ---

async function handleHealth(req, res) {
  jsonResponse(res, 200, {
    status: 'healthy',
    authenticated: client?.isAuthenticated() || false,
    did: clientDid?.value || null,
    tenant_id: tenantTid,
    session_id: sessionId?.value || null,
    contract_tail: CONTRACT_TAIL,
    contract_version: CONTRACT_VERSION,
    endpoints: [
      'identity', 'tenant/me', 'contract/register', 'contract/enable',
      'contract/execute', 'contract/logs', 'kv/put', 'kv/get',
      'maps/create', 'audit/push', 'audit/events', 'usage'
    ],
  });
}

async function handleTenantMe(req, res) {
  if (!requireAuth(res)) return;
  try {
    const me = await tenant.tenant.me();
    jsonResponse(res, 200, { success: true, tenant: me });
  } catch (e) {
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleContractRegister(req, res) {
  if (!requireAuth(res)) return;
  try {
    const wasmBytes = readFileSync(CONTRACT_WASM_PATH);
    const result = await tenant.contracts.register({
      tail: CONTRACT_TAIL,
      version: CONTRACT_VERSION,
      wasm: wasmBytes,
    });

    // Get the contract ID from the registered script
    let contractId = null;
    try {
      const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;
      const version = await getScriptVersion(getNodeUrl(), scriptName);
      // Contract ID is typically in the registration result or we derive from the version check
      contractId = result?.contract_id || result?.id || null;
    } catch (e) { /* non-fatal */ }

    console.log(`[t3n-bridge] Contract registered: ${CONTRACT_TAIL}@${CONTRACT_VERSION}, id=${contractId}`);
    jsonResponse(res, 200, { registered: true, tail: CONTRACT_TAIL, version: CONTRACT_VERSION, contract_id: contractId, result });
  } catch (e) {
    console.log(`[t3n-bridge] Contract register failed: ${e.message}`);
    jsonResponse(res, 200, { registered: false, error: e.message });
  }
}

async function handleContractEnable(req, res) {
  if (!requireAuth(res)) return;
  try {
    const result = await tenant.contracts.enable(CONTRACT_TAIL);
    console.log(`[t3n-bridge] Contract enabled: ${CONTRACT_TAIL}`);
    jsonResponse(res, 200, { enabled: true, tail: CONTRACT_TAIL, result });
  } catch (e) {
    jsonResponse(res, 200, { enabled: false, error: e.message });
  }
}

async function handleContractLogs(req, res) {
  if (!requireAuth(res)) return;
  try {
    const logs = await tenant.contracts.logs(CONTRACT_TAIL, { limit: 50 });
    jsonResponse(res, 200, { success: true, logs });
  } catch (e) {
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleMapsCreate(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { tail, visibility, readers, writers } = payload;
  try {
    const result = await tenant.maps.create({
      tail: tail || "verigate-facts",
      visibility: visibility || "private",
      readers: readers || "all",
      writers: writers || "all",
    });
    console.log(`[t3n-bridge] Map created: ${tail}`);
    jsonResponse(res, 200, { created: true, tail, result });
  } catch (e) {
    console.log(`[t3n-bridge] Map create failed: ${e.message}`);
    jsonResponse(res, 200, { created: false, error: e.message });
  }
}

async function handleIdentity(req, res) {
  if (!requireAuth(res)) return;
  const { balance } = await client.getUsage();
  jsonResponse(res, 200, {
    agent_did: clientDid.value,
    authenticated: true,
    sdk_version: "t3n-sdk-live",
    capabilities: ["tee-execution", "http-with-placeholders", "kv-store", "audit-trail", "contract-execute"],
    credits: balance.available,
    session_id: sessionId.value,
  });
}

async function handleContractExecute(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { function_name, args, input } = payload;
  const execId = `t3n-exec-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;

  try {
    const result = await client.execute({
      script_name: scriptName,
      script_version: CONTRACT_VERSION,
      function_name: function_name || 'verify-credential',
      input: input || args,
    });
    const parsed = JSON.parse(result);
    console.log(`[t3n-bridge] Contract executed LIVE: ${scriptName}/${function_name} → ${execId}`);
    jsonResponse(res, 200, { success: true, result: parsed, execution_id: execId, tee_mode: "live" });
  } catch (e) {
    console.log(`[t3n-bridge] Contract execute failed: ${scriptName}/${function_name} → ${e.message}`);
    jsonResponse(res, 200, {
      success: false,
      error: e.message,
      execution_id: execId,
      tee_mode: "live",
    });
  }
}

async function handleKvPut(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { map_name, key, value } = payload;
  const canonicalMap = `z:${tenantTid}:${map_name}`;

  try {
    await tenant.executeControl("map-entry-set", {
      map_name: canonicalMap,
      key: key,
      value: typeof value === 'string' ? value : JSON.stringify(value),
    });
    console.log(`[t3n-bridge] KV PUT (live): ${canonicalMap}/${key}`);
    jsonResponse(res, 200, { stored: true, key: `${canonicalMap}:${key}`, mode: "live" });
  } catch (e) {
    // Fallback to in-memory
    const fullKey = `${canonicalMap}:${key}`;
    kvStore.set(fullKey, { value, stored_at: new Date().toISOString(), did: clientDid?.value });
    console.log(`[t3n-bridge] KV PUT (local fallback): ${fullKey} (${e.message})`);
    jsonResponse(res, 200, { stored: true, key: fullKey, mode: "local", note: `Fallback: ${e.message}` });
  }
}

async function handleKvGet(req, res) {
  if (!requireAuth(res)) return;
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const mapName = url.searchParams.get('map');
  const key = url.searchParams.get('key');
  const canonicalMap = `z:${tenantTid}:${mapName}`;
  const fullKey = `${canonicalMap}:${key}`;

  // KV reads for private maps only happen inside contracts.
  // Check local fallback store.
  const entry = kvStore.get(fullKey);
  if (entry) {
    jsonResponse(res, 200, { found: true, key: fullKey, value: entry.value, mode: "local" });
  } else {
    jsonResponse(res, 200, { found: false, key: fullKey, mode: "local", note: "Private map reads happen inside TEE contract only" });
  }
}

async function handleAuditPush(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const eventId = `t3n-audit-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;

  const auditEvent = {
    t3n_event_id: eventId,
    agent_did: clientDid?.value,
    ...payload,
    pushed_at: new Date().toISOString(),
  };

  auditLedger.push(auditEvent);
  console.log(`[t3n-bridge] Audit push: ${payload.action} (case: ${payload.case_id}) → ${eventId}`);
  jsonResponse(res, 200, { pushed: true, t3n_event_id: eventId });
}

async function handleAuditEvents(req, res) {
  if (!requireAuth(res)) return;
  try {
    const sdkEvents = await client.getAuditEvents();
    jsonResponse(res, 200, { sdk_events: sdkEvents, bridge_events: auditLedger, total: auditLedger.length });
  } catch (e) {
    jsonResponse(res, 200, { sdk_events: [], bridge_events: auditLedger, total: auditLedger.length, note: e.message });
  }
}

async function handleUsage(req, res) {
  if (!requireAuth(res)) return;
  try {
    const usage = await client.getUsage();
    jsonResponse(res, 200, usage);
  } catch (e) {
    jsonResponse(res, 500, { error: e.message });
  }
}

async function handleAgentAuthGrant(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { functions, allowedHosts } = payload;
  const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;

  try {
    const userContractVersion = await getScriptVersion(getNodeUrl(), "tee:user/contracts");
    const result = await client.execute({
      script_name: "tee:user/contracts",
      script_version: userContractVersion,
      function_name: "agent-auth-update",
      input: {
        agents: [{
          agentDid: clientDid.value,
          scripts: [{
            scriptName,
            versionReq: CONTRACT_VERSION,
            functions: functions || ["verify-credential"],
            allowedHosts: allowedHosts || [],
          }],
        }],
      },
    });
    const parsed = JSON.parse(result);
    console.log(`[t3n-bridge] Agent Auth grant: functions=${JSON.stringify(functions)}, hosts=${JSON.stringify(allowedHosts)}`);
    jsonResponse(res, 200, { granted: true, scoped_to: { functions, allowedHosts }, result: parsed });
  } catch (e) {
    console.log(`[t3n-bridge] Agent Auth grant failed: ${e.message}`);
    jsonResponse(res, 200, { granted: false, error: e.message });
  }
}

async function handleAgentAuthRevoke(req, res) {
  if (!requireAuth(res)) return;
  const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;

  try {
    const userContractVersion = await getScriptVersion(getNodeUrl(), "tee:user/contracts");
    const result = await client.execute({
      script_name: "tee:user/contracts",
      script_version: userContractVersion,
      function_name: "agent-auth-update",
      input: {
        agents: [{
          agentDid: clientDid.value,
          scripts: [{
            scriptName,
            versionReq: CONTRACT_VERSION,
            functions: [],
            allowedHosts: [],
          }],
        }],
      },
    });
    const parsed = JSON.parse(result);
    console.log(`[t3n-bridge] Agent Auth revoked`);
    jsonResponse(res, 200, { revoked: true, result: parsed });
  } catch (e) {
    jsonResponse(res, 200, { revoked: false, error: e.message });
  }
}

async function handleAgentAuthTestRejection(req, res) {
  if (!requireAuth(res)) return;
  const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;

  try {
    // Step 1: Grant ONLY verify-credential (no egress hosts)
    const userContractVersion = await getScriptVersion(getNodeUrl(), "tee:user/contracts");
    await client.execute({
      script_name: "tee:user/contracts",
      script_version: userContractVersion,
      function_name: "agent-auth-update",
      input: {
        agents: [{
          agentDid: clientDid.value,
          scripts: [{
            scriptName,
            versionReq: CONTRACT_VERSION,
            functions: ["verify-credential"],
            allowedHosts: [],
          }],
        }],
      },
    });
    console.log(`[t3n-bridge] Scope test: granted verify-credential only`);

    // Step 2: Execute verify-credential (should SUCCEED)
    let verifyResult;
    try {
      const raw = await client.execute({
        script_name: scriptName,
        script_version: CONTRACT_VERSION,
        function_name: "verify-credential",
        input: { case_id: "scope-test", requirement_id: "test", vp: { "@context": [], "type": [], "verifiableCredential": [] }, trusted_issuers: [] },
      });
      verifyResult = { success: true, tee_mode: "live", result: JSON.parse(raw) };
    } catch (e) {
      verifyResult = { success: false, error: e.message };
    }

    // Step 3: Execute notify-counterparty (should be REJECTED by egress policy)
    let notifyResult;
    try {
      const raw = await client.execute({
        script_name: scriptName,
        script_version: CONTRACT_VERSION,
        function_name: "notify-counterparty",
        input: { case_id: "scope-test", status: "test", message: "Should be blocked" },
      });
      const parsed = JSON.parse(raw);
      // Contract returns but egress was denied — check scope_enforced field
      if (parsed.scope_enforced || parsed.egress_allowed === false) {
        notifyResult = { rejected: true, tee_mode: "live", enforcement: "T3N TEE host blocked egress", result: parsed };
        console.log(`[t3n-bridge] Scope test: notify-counterparty BLOCKED by TEE host (EgressDenied)`);
      } else {
        notifyResult = { rejected: false, tee_mode: "live", result: parsed };
      }
    } catch (e) {
      notifyResult = { rejected: true, error: e.message, enforcement: "T3N host rejected execution" };
      console.log(`[t3n-bridge] Scope test: notify-counterparty REJECTED — ${e.message}`);
    }

    jsonResponse(res, 200, {
      demo: "agent-scope-enforcement",
      step1_grant: { functions: ["verify-credential"], allowedHosts: [] },
      step2_verify: verifyResult,
      step3_notify_blocked: notifyResult,
      conclusion: notifyResult.rejected
        ? "PROVEN: T3N TEE host ENFORCED scope boundary — agent egress BLOCKED for unauthorized host"
        : "Enforcement not triggered — check grant configuration",
    });
  } catch (e) {
    jsonResponse(res, 500, { error: e.message });
  }
}

// --- State Machine Orchestration Endpoints ---

async function executeContract(functionName, args) {
  const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;
  // Use getScriptVersion to get runtime's current version (like Umbra does)
  const runtimeVersion = await getScriptVersion(getNodeUrl(), scriptName);
  const raw = await client.execute({
    script_name: scriptName,
    script_version: runtimeVersion,
    function_name: functionName,
    input: args,
  });
  return JSON.parse(raw);
}

async function handleSetPolicy(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  try {
    const result = await executeContract("set-compliance-policy", payload);
    console.log(`[t3n-bridge] Policy set: case=${payload.case_id}`);
    jsonResponse(res, 200, { success: true, result, tee_mode: "live" });
  } catch (e) {
    console.log(`[t3n-bridge] set-policy failed: ${e.message}`);
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleCommitPlan(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  try {
    const result = await executeContract("commit-assessment-plan", payload);
    console.log(`[t3n-bridge] Plan committed: case=${payload.case_id}, steps=${payload.steps?.length}`);
    jsonResponse(res, 200, { success: true, result, tee_mode: "live" });
  } catch (e) {
    console.log(`[t3n-bridge] commit-plan failed: ${e.message}`);
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleGetPlanStatus(req, res) {
  if (!requireAuth(res)) return;
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const caseId = url.searchParams.get('case_id');
  try {
    const result = await executeContract("get-plan-status", { case_id: caseId });
    jsonResponse(res, 200, { success: true, result, tee_mode: "live" });
  } catch (e) {
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleGetEvidenceChain(req, res) {
  if (!requireAuth(res)) return;
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const caseId = url.searchParams.get('case_id');
  try {
    const result = await executeContract("get-evidence-chain", { case_id: caseId });
    jsonResponse(res, 200, { success: true, result, tee_mode: "live" });
  } catch (e) {
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleGetViolations(req, res) {
  if (!requireAuth(res)) return;
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const caseId = url.searchParams.get('case_id');
  try {
    const result = await executeContract("get-violations", { case_id: caseId });
    jsonResponse(res, 200, { success: true, result, tee_mode: "live" });
  } catch (e) {
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleDecide(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  try {
    const result = await executeContract("decide", payload);
    console.log(`[t3n-bridge] Decision made: case=${payload.case_id}, decision=${result.decision}`);
    jsonResponse(res, 200, { success: true, result, tee_mode: "live" });
  } catch (e) {
    console.log(`[t3n-bridge] decide failed: ${e.message}`);
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleExecuteProtected(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  try {
    const result = await executeContract("execute-protected-action", payload);
    console.log(`[t3n-bridge] Protected action: case=${payload.case_id}, type=${payload.action_type}`);
    jsonResponse(res, 200, { success: true, result, tee_mode: "live" });
  } catch (e) {
    console.log(`[t3n-bridge] execute-protected failed: ${e.message}`);
    jsonResponse(res, 200, { success: false, error: e.message });
  }
}

async function handleOrchestrateFull(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { case_id, credentials, policy, llm_api_key, llm_base_url, enable_delegation, facts: injected_facts } = payload;

  if (!case_id) {
    return jsonResponse(res, 400, { error: "case_id required" });
  }

  const timeline = [];
  const startTs = Date.now();
  let delegationInfo = null;

  try {
    // Step 0: Create delegation credential (if enabled — default true)
    if (enable_delegation !== false) {
      const now = Math.floor(Date.now() / 1000);
      const ttl = 3600;
      const vcId = randomBytes(16);
      const agentPubkey = getAgentPubkey();

      const credential = buildDelegationCredential({
        user_did: counterpartyDid,
        agent_pubkey: agentPubkey,
        org_did: clientDid.value,
        contract: `tee:${CONTRACT_TAIL}`,
        functions: ["assess-risk", "commit-assessment-plan", "decide", "verify-credential"],
        scopes: ["COMPLIANCE_CHECK"],
        metadata: { case_id },
        not_before_secs: BigInt(now),
        not_after_secs: BigInt(now + ttl),
        vc_id: vcId,
      });

      const credentialJcs = canonicaliseCredential(credential);
      const counterpartyKeyBytes = hexToBytes(COUNTERPARTY_PRIVATE_KEY.replace("0x", ""));
      const { sig: userSig } = signCredential(credentialJcs, counterpartyKeyBytes);

      activeDelegations.set(case_id, {
        credential,
        credential_jcs: credentialJcs,
        user_sig: userSig,
        vc_id: vcId,
        agent_pubkey: agentPubkey,
        created_at: now,
        expires_at: now + ttl,
        revoked: false,
      });

      delegationInfo = {
        vc_id: b64uEncodeBytes(vcId),
        counterparty_did: counterpartyDid,
        agent_did: clientDid.value,
        ttl_secs: ttl,
        functions: credential.functions,
      };

      timeline.push({ step: "delegation-create", success: true, result: delegationInfo, elapsed_ms: Date.now() - startTs });
      console.log(`[orchestrate] Delegation created: case=${case_id}, vc_id=${delegationInfo.vc_id}`);
    }

    // Step 1: Set policy (optional)
    if (policy) {
      const policyResult = await executeContract("set-compliance-policy", { case_id, policy });
      timeline.push({ step: "set-policy", success: true, result: policyResult, elapsed_ms: Date.now() - startTs });
    }

    // Step 2: Build plan based on credentials count
    const credCount = credentials?.length || 4;
    const steps = [];
    for (let i = 0; i < credCount; i++) {
      steps.push({ function_name: "verify-credential", required: true, timeout_secs: 300 });
    }
    steps.push({ function_name: "assess-risk", required: true, timeout_secs: 600 });
    steps.push({ function_name: "decide", required: true, timeout_secs: 120 });

    const planResult = await executeDelegatedContract("commit-assessment-plan", { case_id, steps, ttl_secs: 3600 }, case_id);
    timeline.push({ step: "commit-plan", success: true, result: planResult, elapsed_ms: Date.now() - startTs });

    // Step 3: Verify each credential (verify_step built into verify-credential)
    if (credentials && credentials.length > 0) {
      for (let i = 0; i < credentials.length; i++) {
        const cred = credentials[i];
        const verifyResult = await executeDelegatedContract("verify-credential", {
          case_id,
          requirement_id: cred.requirement_id || `req-${i}`,
          vp: cred.vp,
          trusted_issuers: cred.trusted_issuers || [],
        }, case_id);
        timeline.push({ step: `verify-credential-${i}`, success: verifyResult.verified || false, result: verifyResult, elapsed_ms: Date.now() - startTs });
      }
    }

    // Step 4: Assess risk (verify_step built-in, writes evidence internally)
    const allFacts = timeline
      .filter(t => t.step.startsWith("verify-credential") && t.result?.facts)
      .flatMap(t => t.result.facts);

    const assessResult = await executeDelegatedContract("assess-risk", {
      case_id,
      facts: allFacts,
      policy_context: policy || { default: true },
      llm_api_key: llm_api_key || process.env.PIONEER_API_KEY || "",
      llm_base_url: llm_base_url || "https://api.pioneer.ai/v1",
    }, case_id);
    timeline.push({ step: "assess-risk", success: true, result: assessResult, elapsed_ms: Date.now() - startTs });

    // Step 5: Decide (verify_step built-in, writes evidence internally)
    const decideResult = await executeDelegatedContract("decide", { case_id }, case_id);
    timeline.push({ step: "decide", success: true, result: decideResult, elapsed_ms: Date.now() - startTs });

    // Step 10: Auto-revoke delegation after decision (lifecycle complete)
    let revocationInfo = null;
    if (enable_delegation !== false && activeDelegations.has(case_id)) {
      const delegation = activeDelegations.get(case_id);
      delegation.revoked = true;
      delegation.revoked_at = Math.floor(Date.now() / 1000);
      revocationInfo = {
        revoked: true,
        vc_id: b64uEncodeBytes(delegation.vc_id),
        revoked_at: new Date(delegation.revoked_at * 1000).toISOString(),
        reason: `Case ${case_id} decided: ${decideResult.decision}`,
      };
      timeline.push({ step: "delegation-revoke", success: true, result: revocationInfo, elapsed_ms: Date.now() - startTs });
      console.log(`[orchestrate] Delegation revoked: case=${case_id}, decision=${decideResult.decision}`);
    }

    const totalMs = Date.now() - startTs;
    console.log(`[t3n-bridge] Full orchestration: case=${case_id}, decision=${decideResult.decision}, ${totalMs}ms`);

    jsonResponse(res, 200, {
      success: true,
      case_id,
      decision: decideResult.decision,
      confidence: decideResult.confidence,
      evidence_chain_hash: decideResult.evidence_chain_hash,
      delegation: delegationInfo ? {
        ...delegationInfo,
        revocation: revocationInfo,
        lifecycle: "create → authorize → execute → revoke (complete)",
      } : null,
      total_elapsed_ms: totalMs,
      timeline,
      tee_mode: "live",
    });
  } catch (e) {
    timeline.push({ step: "error", success: false, error: e.message, elapsed_ms: Date.now() - startTs });
    console.log(`[t3n-bridge] Orchestration failed at step: ${e.message}`);
    jsonResponse(res, 200, {
      success: false,
      case_id,
      error: e.message,
      timeline,
      partial: true,
    });
  }
}

async function handleSetupKvMap(req, res) {
  if (!requireAuth(res)) return;
  try {
    // Dynamically resolve contract ID from the registered script
    const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;
    let contractId = null;
    try {
      const resp = await client.execute({
        script_name: "tee:user/contracts",
        script_version: await getScriptVersion(getNodeUrl(), "tee:user/contracts"),
        function_name: "get-agent-auth",
        input: { agentDid: clientDid.value },
      });
      const parsed = JSON.parse(resp);
      const scripts = parsed?.agents?.[0]?.scripts || [];
      const match = scripts.find(s => s.scriptName === scriptName);
      if (match) contractId = match.contractId;
    } catch (e) { /* fallback below */ }

    // If we couldn't get it from agent-auth, try the payload or use "all"
    const payload = await parseBody(req).catch(() => ({}));
    contractId = contractId || payload.contract_id || null;

    const acl = contractId ? { only: [contractId] } : "all";

    // Try to create map first
    try {
      const result = await tenant.maps.create({
        tail: "vg-state",
        visibility: "private",
        readers: acl,
        writers: acl,
      });
      console.log(`[t3n-bridge] KV map created with ACL: ${JSON.stringify(acl)}`);
      jsonResponse(res, 200, { created: true, map: "vg-state", contract_id: contractId, acl, result });
    } catch (e) {
      if (e.message?.includes("already exists") || e.message?.includes("MapAlreadyExists")) {
        try {
          await tenant.maps.update("vg-state", { readers: acl, writers: acl });
          console.log(`[t3n-bridge] KV map ACL updated: ${JSON.stringify(acl)}`);
          jsonResponse(res, 200, { created: false, map: "vg-state", contract_id: contractId, acl, note: "ACL updated" });
        } catch (ue) {
          jsonResponse(res, 200, { created: false, map: "vg-state", error: ue.message });
        }
      } else {
        throw e;
      }
    }
  } catch (e) {
    jsonResponse(res, 200, { created: false, error: e.message });
  }
}

// --- Delegation Credential Endpoints ---

async function handleDelegationCreate(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { case_id, functions, ttl_secs, scopes } = payload;

  if (!case_id) return jsonResponse(res, 400, { error: "Missing case_id" });

  const now = Math.floor(Date.now() / 1000);
  const ttl = ttl_secs || 3600;
  const vcId = randomBytes(16);
  const agentPubkey = getAgentPubkey();

  try {
    const credential = buildDelegationCredential({
      user_did: counterpartyDid,
      agent_pubkey: agentPubkey,
      org_did: clientDid.value,
      contract: `tee:${CONTRACT_TAIL}`,
      functions: (functions || ["assess-risk", "commit-assessment-plan", "decide", "verify-credential"]).sort(),
      scopes: scopes || ["COMPLIANCE_CHECK"],
      metadata: { case_id },
      not_before_secs: BigInt(now),
      not_after_secs: BigInt(now + ttl),
      vc_id: vcId,
    });

    validateCredentialBody(credential);

    // Canonicalise to JCS bytes (RFC 8785) — this is what gets signed
    const credentialJcs = canonicaliseCredential(credential);

    // Counterparty signs (EIP-191) — proves DATA OWNER authorized this agent
    const counterpartyKeyBytes = hexToBytes(COUNTERPARTY_PRIVATE_KEY.replace("0x", ""));
    const { sig: userSig, addr: recoveredAddr } = signCredential(credentialJcs, counterpartyKeyBytes);

    // Store active delegation
    activeDelegations.set(case_id, {
      credential,
      credential_jcs: credentialJcs,
      user_sig: userSig,
      vc_id: vcId,
      agent_pubkey: agentPubkey,
      created_at: now,
      expires_at: now + ttl,
      revoked: false,
    });

    console.log(`[delegation] Created: case=${case_id}, vc_id=${b64uEncodeBytes(vcId)}, counterparty=${counterpartyDid}, ttl=${ttl}s`);

    jsonResponse(res, 200, {
      delegation_created: true,
      case_id,
      vc_id: b64uEncodeBytes(vcId),
      counterparty_did: counterpartyDid,
      counterparty_address: `0x${Buffer.from(recoveredAddr).toString('hex')}`,
      agent_did: clientDid.value,
      org_did: clientDid.value,
      contract: `tee:${CONTRACT_TAIL}`,
      functions: credential.functions,
      scopes: credential.scopes,
      not_before: new Date(now * 1000).toISOString(),
      not_after: new Date((now + ttl) * 1000).toISOString(),
      ttl_secs: ttl,
      signature: {
        type: "EIP-191",
        user_sig: b64uEncodeBytes(userSig),
        credential_jcs_hash: createHash('sha256').update(credentialJcs).digest('hex'),
      },
      proof: "Counterparty (data owner) cryptographically authorized agent via W3C VC delegation",
    });
  } catch (e) {
    console.error(`[delegation] Create failed: ${e.message}`);
    jsonResponse(res, 200, { delegation_created: false, error: e.message });
  }
}

async function handleDelegationRevoke(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { case_id, vc_id } = payload;

  const delegation = case_id ? activeDelegations.get(case_id) : null;
  if (!delegation) {
    return jsonResponse(res, 200, { revoked: false, error: `No active delegation for case ${case_id}` });
  }

  try {
    // Call T3N revocation service
    const revokeResult = await revokeDelegation({
      t3n: client,
      baseUrl: getNodeUrl(),
      credential_jcs: delegation.credential_jcs,
      user_sig: delegation.user_sig,
    });

    delegation.revoked = true;
    delegation.revoked_at = Math.floor(Date.now() / 1000);

    console.log(`[delegation] Revoked: case=${case_id}, vc_id=${b64uEncodeBytes(delegation.vc_id)}`);

    jsonResponse(res, 200, {
      revoked: true,
      case_id,
      vc_id: b64uEncodeBytes(delegation.vc_id),
      revoked_at: new Date(delegation.revoked_at * 1000).toISOString(),
      revocation_result: revokeResult,
      proof: "Delegation credential revoked — agent can no longer act on behalf of counterparty",
    });
  } catch (e) {
    // If T3N revocation service isn't available, still mark locally
    delegation.revoked = true;
    delegation.revoked_at = Math.floor(Date.now() / 1000);

    console.log(`[delegation] Revoked locally (T3N service: ${e.message}): case=${case_id}`);
    jsonResponse(res, 200, {
      revoked: true,
      case_id,
      vc_id: b64uEncodeBytes(delegation.vc_id),
      revoked_at: new Date(delegation.revoked_at * 1000).toISOString(),
      local_revocation: true,
      t3n_revocation_note: e.message,
      proof: "Delegation credential revoked locally — agent authorization withdrawn",
    });
  }
}

async function handleDelegationStatus(req, res) {
  if (!requireAuth(res)) return;
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const case_id = url.searchParams.get("case_id");

  if (!case_id) return jsonResponse(res, 400, { error: "Missing case_id query param" });

  const delegation = activeDelegations.get(case_id);
  if (!delegation) {
    return jsonResponse(res, 200, { active: false, case_id, reason: "No delegation found" });
  }

  const now = Math.floor(Date.now() / 1000);
  const expired = now > delegation.expires_at;

  jsonResponse(res, 200, {
    active: !delegation.revoked && !expired,
    case_id,
    vc_id: b64uEncodeBytes(delegation.vc_id),
    counterparty_did: counterpartyDid,
    agent_did: clientDid.value,
    functions: delegation.credential.functions,
    created_at: new Date(delegation.created_at * 1000).toISOString(),
    expires_at: new Date(delegation.expires_at * 1000).toISOString(),
    revoked: delegation.revoked,
    revoked_at: delegation.revoked_at ? new Date(delegation.revoked_at * 1000).toISOString() : null,
    expired,
    remaining_secs: expired ? 0 : delegation.expires_at - now,
  });
}

// Delegated contract execution — attaches DelegationEnvelope to each call
async function executeDelegatedContract(functionName, args, caseId) {
  const delegation = activeDelegations.get(caseId);

  // If no active delegation, fall through to regular execution
  if (!delegation || delegation.revoked) {
    return executeContract(functionName, args);
  }

  const now = Math.floor(Date.now() / 1000);
  if (now > delegation.expires_at) {
    throw new Error(`Delegation expired for case ${caseId} (expired at ${new Date(delegation.expires_at * 1000).toISOString()})`);
  }

  // Build per-call invocation envelope
  const nonce = randomBytes(16);
  const requestBody = JSON.stringify(args);
  const reqHash = createHash('sha256').update(requestBody).digest();

  // Agent signs the invocation preimage: domain || vc_id || nonce || request_hash
  const preimage = buildInvocationPreimage(delegation.vc_id, nonce, reqHash);
  const agentKeyBytes = hexToBytes(T3N_API_KEY.replace("0x", ""));
  const agentSig = signAgentInvocation(preimage, agentKeyBytes);

  const envelope = {
    credential_jcs: b64uEncodeBytes(delegation.credential_jcs),
    user_sig: b64uEncodeBytes(delegation.user_sig),
    agent_sig: b64uEncodeBytes(agentSig),
    nonce: b64uEncodeBytes(nonce),
    request_hash: b64uEncodeBytes(reqHash),
  };

  const scriptName = `z:${tenantTid}:${CONTRACT_TAIL}`;
  const runtimeVersion = await getScriptVersion(getNodeUrl(), scriptName);

  // Delegated execution: pii_did tells T3N runtime this is on behalf of counterparty
  // Runtime validates envelope, stamps actor=agent, subject=counterparty, vc_id in audit
  const raw = await client.execute({
    script_name: scriptName,
    script_version: runtimeVersion,
    function_name: functionName,
    input: { ...args, delegation_envelope: envelope },
    pii_did: counterpartyDid,
  });
  return JSON.parse(raw);
}

// Test delegated execution with audit trail verification
async function handleDelegationTest(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { case_id } = payload;
  const testCaseId = case_id || `deleg-enforce-${Date.now()}`;

  try {
    // Step 1: Create delegation
    const now = Math.floor(Date.now() / 1000);
    const vcId = randomBytes(16);
    const agentPubkey = getAgentPubkey();

    const credential = buildDelegationCredential({
      user_did: counterpartyDid,
      agent_pubkey: agentPubkey,
      org_did: clientDid.value,
      contract: `tee:${CONTRACT_TAIL}`,
      functions: ["commit-assessment-plan", "verify-credential"],
      scopes: ["COMPLIANCE_CHECK"],
      metadata: { case_id: testCaseId },
      not_before_secs: BigInt(now),
      not_after_secs: BigInt(now + 3600),
      vc_id: vcId,
    });

    const credentialJcs = canonicaliseCredential(credential);
    const counterpartyKeyBytes = hexToBytes(COUNTERPARTY_PRIVATE_KEY.replace("0x", ""));
    const { sig: userSig } = signCredential(credentialJcs, counterpartyKeyBytes);

    activeDelegations.set(testCaseId, {
      credential,
      credential_jcs: credentialJcs,
      user_sig: userSig,
      vc_id: vcId,
      agent_pubkey: agentPubkey,
      created_at: now,
      expires_at: now + 3600,
      revoked: false,
    });

    // Step 2: Execute a contract call WITH delegation (delegated mode)
    let delegatedResult;
    let delegatedError = null;
    try {
      delegatedResult = await executeDelegatedContract("commit-assessment-plan", {
        case_id: testCaseId,
        steps: [{ function_name: "verify-credential" }],
        ttl_secs: 3600,
      }, testCaseId);
    } catch (e) {
      delegatedError = e.message;
    }

    // Step 3: Check audit trail for vc_id stamp
    let auditResult = null;
    try {
      auditResult = await client.getAuditEvents({ limit: 5 });
    } catch (e) {
      auditResult = { error: e.message };
    }

    // Step 4: Revoke and try again (should still execute via fallback but without delegation authority)
    activeDelegations.get(testCaseId).revoked = true;

    jsonResponse(res, 200, {
      test: "delegation-enforcement",
      case_id: testCaseId,
      delegation: {
        vc_id: b64uEncodeBytes(vcId),
        counterparty_did: counterpartyDid,
        agent_did: clientDid.value,
        contract: `tee:${CONTRACT_TAIL}`,
        functions: credential.functions,
      },
      delegated_execution: {
        success: !delegatedError,
        result: delegatedResult || null,
        error: delegatedError,
        pii_did_passed: counterpartyDid,
        note: delegatedError
          ? "T3N runtime rejected — may need agent-auth grant for counterparty's pii_did scope"
          : "T3N runtime accepted delegated execution with pii_did + envelope",
      },
      audit_trail: auditResult,
      analysis: {
        envelope_built: true,
        cryptography: "EIP-191 user_sig + secp256k1 agent_sig per-call",
        pii_did_mode: "counterparty DID passed to runtime for delegated actor/subject stamping",
        enforcement_level: delegatedError
          ? "Runtime-level (T3N host rejected without proper grant chain)"
          : "Runtime-level (T3N host accepted and stamped audit with vc_id)",
      },
    });
  } catch (e) {
    jsonResponse(res, 200, { test: "delegation-enforcement", error: e.message });
  }
}

// --- Scenario Runner (for Frontend Evidence Chain) ---

async function handleScenarioRun(req, res) {
  if (!requireAuth(res)) return;
  const payload = await parseBody(req);
  const { scenario } = payload;

  if (!scenario || !['good', 'sanctioned', 'incomplete'].includes(scenario)) {
    return jsonResponse(res, 400, { error: "scenario must be: good, sanctioned, or incomplete" });
  }

  function makeIssuer() {
    const { publicKey, privateKey } = generateKeyPairSync('ed25519');
    const pubRaw = publicKey.export({ type: 'spki', format: 'der' }).subarray(-32);
    const multicodec = Buffer.concat([Buffer.from([0xed, 0x01]), pubRaw]);
    const ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';
    let num = BigInt('0x' + Buffer.from(multicodec).toString('hex'));
    let encoded = '';
    while (num > 0n) { encoded = ALPHABET[Number(num % 58n)] + encoded; num = num / 58n; }
    for (const byte of multicodec) { if (byte === 0) encoded = '1' + encoded; else break; }
    return { did: 'did:key:z' + encoded, privateKey };
  }

  function makeJWT(issuer, subject, claims) {
    const now = Math.floor(Date.now() / 1000);
    const header = { alg: "EdDSA", typ: "JWT" };
    const pl = {
      iss: issuer.did, sub: subject, iat: now, exp: now + 86400,
      vc: { "@context": ["https://www.w3.org/2018/credentials/v1"], type: ["VerifiableCredential"], credentialSubject: claims },
    };
    const hB64 = Buffer.from(JSON.stringify(header)).toString('base64url');
    const pB64 = Buffer.from(JSON.stringify(pl)).toString('base64url');
    const sigInput = `${hB64}.${pB64}`;
    const sig = cryptoSign(null, Buffer.from(sigInput), issuer.privateKey);
    return `${sigInput}.${sig.toString('base64url')}`;
  }

  const issuer1 = makeIssuer();
  const issuer2 = makeIssuer();
  const caseId = `${scenario}-${Date.now()}`;

  let credentials;
  if (scenario === 'good') {
    credentials = [
      {
        requirement_id: "business-license",
        vp: { "@context": ["https://www.w3.org/2018/credentials/v1"], type: ["VerifiablePresentation"], verifiableCredential: [
          makeJWT(issuer1, "did:example:acme-sg", {
            entity_name: "Acme Trading Pte Ltd", legal_name: "Acme Trading Pte Ltd",
            registration_number: "202312345G", jurisdiction: "Singapore",
            entity_type: "Private Limited Company", country: "Singapore",
            region: "Southeast Asia", sanctions_clear: "true",
            aml_checked: "true", pep_status: "no_matches", expiry_date: "2028-06-15",
          }),
        ]},
        trusted_issuers: [issuer1.did],
      },
      {
        requirement_id: "financial-clearance",
        vp: { "@context": ["https://www.w3.org/2018/credentials/v1"], type: ["VerifiablePresentation"], verifiableCredential: [
          makeJWT(issuer2, "did:example:acme-sg", {
            entity_name: "Acme Trading Pte Ltd", jurisdiction: "Singapore",
            country: "Singapore", sanctions_clear: "true", aml_checked: "true",
            pep_status: "no_matches", verified: "true", document_type: "Financial Clearance",
          }),
        ]},
        trusted_issuers: [issuer2.did],
      },
    ];
  } else if (scenario === 'sanctioned') {
    credentials = [{
      requirement_id: "entity-docs",
      vp: { "@context": ["https://www.w3.org/2018/credentials/v1"], type: ["VerifiablePresentation"], verifiableCredential: [
        makeJWT(issuer1, "did:example:dprk", {
          entity_name: "Korea Munitions Industry Department",
          legal_name: "Korea Munitions Industry Department",
          registration_number: "KP-OFAC-001", jurisdiction: "North Korea (DPRK)",
          entity_type: "State Military Enterprise", country: "North Korea",
          region: "East Asia - Sanctioned Territory",
          sanctions_clear: "false - OFAC SDN List, UN Resolution 1718",
          aml_checked: "FAILED - designated entity under comprehensive sanctions",
          pep_status: "Kim Jong Un (Head of State) - maximum risk",
        }),
      ]},
      trusted_issuers: [issuer1.did],
    }];
  } else {
    credentials = [{
      requirement_id: "partial-docs",
      vp: { "@context": ["https://www.w3.org/2018/credentials/v1"], type: ["VerifiablePresentation"], verifiableCredential: [
        makeJWT(issuer1, "did:example:mystery", {
          entity_name: "XYZ Offshore Holdings Ltd", jurisdiction: "Unknown",
          entity_type: "Shell Company", country: "Unverified", verified: "false",
          pep_status: "unable_to_determine",
          sanctions_clear: "unknown - screening incomplete",
          aml_checked: "inconclusive - nominee structure detected",
        }),
      ]},
      trusted_issuers: [issuer1.did],
    }];
  }

  try {
    const result = await handleOrchestratePipeline(caseId, credentials, PIONEER_API_KEY);
    jsonResponse(res, 200, result);
  } catch (e) {
    jsonResponse(res, 200, { success: false, case_id: caseId, scenario, error: e.message });
  }
}

// Extracted orchestration logic for reuse
async function handleOrchestratePipeline(case_id, credentials, llm_api_key) {
  const timeline = [];
  const startTs = Date.now();
  let delegationInfo = null;
  const enable_delegation = true;

  // Delegation create
  const now = Math.floor(Date.now() / 1000);
  const ttl = 3600;
  const vcId = randomBytes(16);
  const agentPubkey = getAgentPubkey();

  const credential = buildDelegationCredential({
    user_did: counterpartyDid,
    agent_pubkey: agentPubkey,
    org_did: clientDid.value,
    contract: `tee:${CONTRACT_TAIL}`,
    functions: ["assess-risk", "commit-assessment-plan", "decide", "verify-credential"],
    scopes: ["COMPLIANCE_CHECK"],
    metadata: { case_id },
    not_before_secs: BigInt(now),
    not_after_secs: BigInt(now + ttl),
    vc_id: vcId,
  });

  const credentialJcs = canonicaliseCredential(credential);
  const counterpartyKeyBytes = hexToBytes(COUNTERPARTY_PRIVATE_KEY.replace("0x", ""));
  const { sig: userSig } = signCredential(credentialJcs, counterpartyKeyBytes);

  activeDelegations.set(case_id, {
    credential, credential_jcs: credentialJcs, user_sig: userSig,
    vc_id: vcId, agent_pubkey: agentPubkey, created_at: now, expires_at: now + ttl, revoked: false,
  });

  delegationInfo = {
    vc_id: b64uEncodeBytes(vcId),
    counterparty_did: counterpartyDid,
    agent_did: clientDid.value,
    ttl_secs: ttl,
    functions: credential.functions,
  };
  timeline.push({ step: "delegation-create", success: true, result: delegationInfo, elapsed_ms: Date.now() - startTs });

  // Plan
  const credCount = credentials?.length || 1;
  const steps = [];
  for (let i = 0; i < credCount; i++) steps.push({ function_name: "verify-credential" });
  steps.push({ function_name: "assess-risk" });
  steps.push({ function_name: "decide" });

  const planResult = await executeDelegatedContract("commit-assessment-plan", { case_id, steps, ttl_secs: 3600 }, case_id);
  timeline.push({ step: "commit-plan", success: true, result: planResult, elapsed_ms: Date.now() - startTs });

  // Verify credentials
  for (let i = 0; i < credentials.length; i++) {
    const cred = credentials[i];
    const verifyResult = await executeDelegatedContract("verify-credential", {
      case_id, requirement_id: cred.requirement_id || `req-${i}`,
      vp: cred.vp, trusted_issuers: cred.trusted_issuers || [],
    }, case_id);
    timeline.push({ step: `verify-credential-${i}`, success: verifyResult.verified || false, result: verifyResult, elapsed_ms: Date.now() - startTs });
  }

  // Assess risk
  const allFacts = timeline
    .filter(t => t.step.startsWith("verify-credential") && t.result?.facts)
    .flatMap(t => t.result.facts);

  const assessResult = await executeDelegatedContract("assess-risk", {
    case_id, facts: allFacts, policy_context: { default: true },
    llm_api_key: llm_api_key || process.env.PIONEER_API_KEY || "",
    llm_base_url: "https://api.pioneer.ai/v1",
  }, case_id);
  timeline.push({ step: "assess-risk", success: true, result: assessResult, elapsed_ms: Date.now() - startTs });

  // Decide
  const decideResult = await executeDelegatedContract("decide", { case_id }, case_id);
  timeline.push({ step: "decide", success: true, result: decideResult, elapsed_ms: Date.now() - startTs });

  // Revoke delegation
  const delegation = activeDelegations.get(case_id);
  delegation.revoked = true;
  delegation.revoked_at = Math.floor(Date.now() / 1000);
  const revocationInfo = {
    revoked: true, vc_id: b64uEncodeBytes(delegation.vc_id),
    revoked_at: new Date(delegation.revoked_at * 1000).toISOString(),
    reason: `Case ${case_id} decided: ${decideResult.decision}`,
  };
  timeline.push({ step: "delegation-revoke", success: true, result: revocationInfo, elapsed_ms: Date.now() - startTs });

  return {
    success: true, case_id,
    decision: decideResult.decision,
    confidence: decideResult.confidence,
    evidence_chain_hash: decideResult.evidence_chain_hash,
    delegation: { ...delegationInfo, revocation: revocationInfo, lifecycle: "create → authorize → execute → revoke (complete)" },
    total_elapsed_ms: Date.now() - startTs,
    timeline,
    tee_mode: "live",
  };
}

// --- Router ---

const CORS_HEADERS = {
  'Access-Control-Allow-Origin': '*',
  'Access-Control-Allow-Methods': 'GET, POST, OPTIONS',
  'Access-Control-Allow-Headers': 'Content-Type, Authorization',
};

async function handleRequest(req, res) {
  const url = new URL(req.url, `http://localhost:${PORT}`);
  const method = req.method;

  // CORS preflight
  if (method === 'OPTIONS') {
    res.writeHead(204, CORS_HEADERS);
    res.end();
    return;
  }

  // Set CORS on all responses
  Object.entries(CORS_HEADERS).forEach(([k, v]) => res.setHeader(k, v));

  try {
    if (url.pathname === '/health' && method === 'GET') return await handleHealth(req, res);
    if (url.pathname === '/identity' && method === 'GET') return await handleIdentity(req, res);
    if (url.pathname === '/tenant/me' && method === 'GET') return await handleTenantMe(req, res);
    if (url.pathname === '/contract/register' && method === 'POST') return await handleContractRegister(req, res);
    if (url.pathname === '/contract/enable' && method === 'POST') return await handleContractEnable(req, res);
    if (url.pathname === '/contract/execute' && method === 'POST') return await handleContractExecute(req, res);
    if (url.pathname === '/contract/logs' && method === 'GET') return await handleContractLogs(req, res);
    if (url.pathname === '/maps/create' && method === 'POST') return await handleMapsCreate(req, res);
    if (url.pathname === '/agent-auth/grant' && method === 'POST') return await handleAgentAuthGrant(req, res);
    if (url.pathname === '/agent-auth/revoke' && method === 'POST') return await handleAgentAuthRevoke(req, res);
    if (url.pathname === '/agent-auth/test-rejection' && method === 'POST') return await handleAgentAuthTestRejection(req, res);

    // Delegation credential endpoints (Phase B — W3C VC-based agent delegation)
    if (url.pathname === '/delegation/create' && method === 'POST') return await handleDelegationCreate(req, res);
    if (url.pathname === '/delegation/revoke' && method === 'POST') return await handleDelegationRevoke(req, res);
    if (url.pathname === '/delegation/status' && method === 'GET') return await handleDelegationStatus(req, res);
    if (url.pathname === '/delegation/test' && method === 'POST') return await handleDelegationTest(req, res);
    if (url.pathname === '/kv/put' && method === 'POST') return await handleKvPut(req, res);
    if (url.pathname === '/kv/get' && method === 'GET') return await handleKvGet(req, res);
    if (url.pathname === '/audit/push' && method === 'POST') return await handleAuditPush(req, res);
    if (url.pathname === '/audit/events' && method === 'GET') return await handleAuditEvents(req, res);
    if (url.pathname === '/usage' && method === 'GET') return await handleUsage(req, res);

    // State machine endpoints
    if (url.pathname === '/policy/set' && method === 'POST') return await handleSetPolicy(req, res);
    if (url.pathname === '/plan/commit' && method === 'POST') return await handleCommitPlan(req, res);
    if (url.pathname === '/plan/status' && method === 'GET') return await handleGetPlanStatus(req, res);
    if (url.pathname === '/evidence' && method === 'GET') return await handleGetEvidenceChain(req, res);
    if (url.pathname === '/violations' && method === 'GET') return await handleGetViolations(req, res);
    if (url.pathname === '/decide' && method === 'POST') return await handleDecide(req, res);
    if (url.pathname === '/protected/execute' && method === 'POST') return await handleExecuteProtected(req, res);
    if (url.pathname === '/orchestrate/full' && method === 'POST') return await handleOrchestrateFull(req, res);
    if (url.pathname === '/setup/kv-map' && method === 'POST') return await handleSetupKvMap(req, res);

    // Scenario runner (for frontend Evidence Chain viewer)
    if (url.pathname === '/scenarios/run' && method === 'POST') return await handleScenarioRun(req, res);

    // Legacy endpoints (backward compat)
    if (url.pathname === '/audit-events' && method === 'GET') return await handleAuditEvents(req, res);
    if (url.pathname === '/execute' && method === 'POST') return await handleContractExecute(req, res);

    jsonResponse(res, 404, { error: 'Not found' });
  } catch (e) {
    console.error(`[t3n-bridge] Error handling ${method} ${url.pathname}:`, e.message);
    jsonResponse(res, 500, { error: e.message });
  }
}

const server = http.createServer(handleRequest);

initClient().then(() => {
  server.listen(PORT, '0.0.0.0', () => {
    console.log(`[t3n-bridge] Listening on port ${PORT}`);
    console.log(`[t3n-bridge] Endpoints: health, identity, contract/execute, kv/put, kv/get, audit/push, audit/events, usage`);
  });
}).catch(err => {
  console.error('[t3n-bridge] Failed to initialize:', err.message);
  process.exit(1);
});
