const BACKEND_HEALTH_URL = 'http://localhost:3002/api/health';
const POLL_INTERVAL_MS = 2_000;
const MAX_WAIT_MS = 90_000;

/**
 * Global setup that polls the backend health endpoint until it responds healthy.
 * Ensures docker-compose services are ready before tests run.
 */
async function globalSetup(): Promise<void> {
  const startTime = Date.now();

  console.log(`[global-setup] Waiting for backend at ${BACKEND_HEALTH_URL}...`);

  while (Date.now() - startTime < MAX_WAIT_MS) {
    try {
      const response = await fetch(BACKEND_HEALTH_URL);

      if (response.ok) {
        const body = await response.json();

        if (body.status === 'healthy') {
          console.log(`[global-setup] Backend ready (${Date.now() - startTime}ms)`);
          return;
        }
      }
    } catch {
      // Service not yet available — keep polling
    }

    await new Promise((resolve) => setTimeout(resolve, POLL_INTERVAL_MS));
  }

  throw new Error(
    `Backend did not become healthy within ${MAX_WAIT_MS / 1_000} seconds. ` +
      'Ensure docker-compose up is running.'
  );
}

export default globalSetup;
