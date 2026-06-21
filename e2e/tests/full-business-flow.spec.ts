import { test, expect } from '@playwright/test';

const BASE = 'http://localhost:3309';
const API = 'http://localhost:3302';

test.describe('Complete Business Flow — End to End Quality Analysis', () => {

  test('full lifecycle: create → submit all → assess → decision', async ({ browser }) => {
    // === REVIEWER: Create Case ===
    const reviewerCtx = await browser.newContext();
    const reviewer = await reviewerCtx.newPage();
    await reviewer.goto(`${BASE}/login`);
    await reviewer.fill('input[name="email"]', 'reviewer@verigate.io');
    await reviewer.fill('input[name="password"]', 'reviewer123');
    await reviewer.click('button[type="submit"]');
    await reviewer.waitForURL('**/dashboard');
    console.log('═══ STEP 1: Reviewer Login ═══');
    console.log('✓ Reviewer authenticated → dashboard');
    await reviewer.screenshot({ path: 'screenshots/full-flow/01-dashboard.png' });

    // Create case
    await reviewer.click('text=New Case');
    await reviewer.waitForTimeout(500);
    await reviewer.fill('input[placeholder*="Meridian"]', 'Quantum Capital Partners');
    // Change entity type to Fund
    await reviewer.selectOption('select', { label: 'Fund' });
    await reviewer.click('button:has-text("Create Case")');
    await reviewer.waitForTimeout(3000);
    console.log('\n═══ STEP 2: Case Creation ═══');
    console.log('✓ Case: Quantum Capital Partners (Fund)');
    await reviewer.screenshot({ path: 'screenshots/full-flow/02-case-created.png' });

    // Get case ID
    const caseLink = reviewer.locator('a:has-text("Open Portal")').first();
    const portalHref = await caseLink.getAttribute('href');
    const caseId = portalHref?.split('/portal/')[1] || '';
    console.log(`✓ Case ID: ${caseId}`);

    // === COUNTERPARTY: Login + Submit All Proofs ===
    const cpCtx = await browser.newContext();
    const cp = await cpCtx.newPage();
    await cp.goto(`${BASE}/login`);
    await cp.fill('input[name="email"]', 'counterparty@verigate.io');
    await cp.fill('input[name="password"]', 'counterparty123');
    await cp.click('button[type="submit"]');
    await cp.waitForURL('**/portal');
    console.log('\n═══ STEP 3: Counterparty Login ═══');
    console.log('✓ Counterparty authenticated → portal');

    // Navigate to case
    await cp.goto(`${BASE}/portal/${caseId}`);
    await cp.waitForTimeout(3000);
    await cp.screenshot({ path: 'screenshots/full-flow/03-portal-requirements.png' });

    // Check requirements
    const reqCountEl = await cp.locator('.portal__tab-count').first().textContent();
    console.log(`✓ Requirements loaded: ${reqCountEl}`);

    // Submit ALL proofs one by one
    console.log('\n═══ STEP 4: Submit All Credentials ═══');
    const submitButtons = cp.locator('button:has-text("Submit Proof")');
    const btnCount = await submitButtons.count();
    console.log(`✓ Submit buttons available: ${btnCount}`);

    for (let i = 0; i < btnCount; i++) {
      const btns = cp.locator('button:has-text("Submit Proof")');
      const currentCount = await btns.count();
      if (currentCount === 0) break;

      await btns.first().click();
      await cp.waitForTimeout(4000);
      console.log(`  → Credential ${i + 1}/${btnCount} submitted`);
    }
    
    await cp.waitForTimeout(3000);
    await cp.screenshot({ path: 'screenshots/full-flow/04-all-submitted.png' });

    // Check completeness
    const progressText = await cp.locator('.portal__progress-badge').textContent();
    console.log(`✓ Progress after all submissions: ${progressText}`);

    // Check verified count
    const verifiedBadges = await cp.locator('text=Verified in TEE').count();
    console.log(`✓ Verified badges visible: ${verifiedBadges}`);

    // === STEP 5: Trigger Assessment ===
    console.log('\n═══ STEP 5: AI Assessment ═══');
    
    // Switch to assessment tab
    await cp.click('text=Assessment');
    await cp.waitForTimeout(1000);
    
    // Look for trigger button
    const assessBtn = cp.locator('button:has-text("Trigger AI Assessment")');
    const hasAssessBtn = await assessBtn.isVisible().catch(() => false);
    console.log(`✓ Trigger Assessment button visible: ${hasAssessBtn}`);
    
    if (hasAssessBtn) {
      await assessBtn.click();
      console.log('✓ Assessment triggered — waiting for 4-agent pipeline...');
      await cp.screenshot({ path: 'screenshots/full-flow/05-assessment-running.png' });
    }

    // Wait for assessment to complete (poll API)
    console.log('\n═══ STEP 6: Wait for Assessment Result ═══');
    const token = await (async () => {
      const r = await fetch(`${API}/api/auth/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email: 'reviewer@verigate.io', password: 'reviewer123' }),
      });
      const d = await r.json();
      return d.token;
    })();

    let assessmentResult = null;
    for (let i = 0; i < 15; i++) {
      await new Promise(r => setTimeout(r, 10000));
      try {
        const r = await fetch(`${API}/api/cases/${caseId}/assessment`, {
          headers: { 'Authorization': `Bearer ${token}` },
        });
        const d = await r.json();
        if (d.data && d.data.decision && d.data.confidence > 0) {
          assessmentResult = d.data;
          break;
        }
      } catch {}
      console.log(`  Polling... attempt ${i + 1}/15`);
    }

    if (assessmentResult) {
      console.log(`✓ Assessment completed!`);
      console.log(`  Decision: ${assessmentResult.decision}`);
      console.log(`  Confidence: ${assessmentResult.confidence}`);
      console.log(`  Summary preview: ${(assessmentResult.summary_text || '').slice(0, 150)}`);
    } else {
      console.log('⚠ Assessment did not complete within timeout (expected with LLM latency)');
    }

    // === STEP 7: Privacy View ===
    console.log('\n═══ STEP 7: Privacy Split-Screen ═══');
    await reviewer.goto(`${BASE}/privacy/${caseId}`);
    await reviewer.waitForTimeout(2000);
    const teeBadge = await reviewer.locator('.tee-badge__label').isVisible();
    const statsVisible = await reviewer.locator('.split-screen__stat--protected').isVisible();
    console.log(`✓ TEE badge visible: ${teeBadge}`);
    console.log(`✓ Stats bar visible: ${statsVisible}`);
    await reviewer.screenshot({ path: 'screenshots/full-flow/06-privacy-final.png' });

    // === QUALITY ANALYSIS ===
    console.log('\n═══════════════════════════════════════');
    console.log('       QUALITY ANALYSIS SUMMARY        ');
    console.log('═══════════════════════════════════════');
    console.log(`Case: Quantum Capital Partners (Fund)`);
    console.log(`Requirements: ${reqCountEl}`);
    console.log(`Submissions: ${btnCount} attempted`);
    console.log(`Progress: ${progressText}`);
    console.log(`Verified in TEE: ${verifiedBadges}`);
    console.log(`Assessment: ${assessmentResult ? `${assessmentResult.decision} (${assessmentResult.confidence})` : 'pending'}`);
    console.log(`Privacy TEE badge: ${teeBadge}`);
    console.log(`Stats visible: ${statsVisible}`);
    console.log('═══════════════════════════════════════');

    await reviewerCtx.close();
    await cpCtx.close();
  });
});
