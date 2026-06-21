import { useEffect, useRef, useCallback } from 'react';
import type { SSEEvent, SSEEventType } from '@/types/portal';

const API_BASE_URL = import.meta.env.VITE_API_URL ?? 'http://localhost:3001';

/** Connection states for the SSE stream. */
export type ConnectionState = 'connecting' | 'connected' | 'disconnected';

interface UseEventStreamOptions {
  /** Case ID to subscribe to. */
  readonly caseId: string;
  /** Callback invoked for each received SSE event. */
  readonly onEvent: (event: SSEEvent) => void;
  /** Whether to enable the connection. Defaults to true. */
  readonly enabled?: boolean;
}

/** Known event types the stream emits. */
const SSE_EVENT_TYPES: readonly SSEEventType[] = [
  'requirement_added',
  'submission_verified',
  'assessment_complete',
  'status_changed',
];

const INITIAL_BACKOFF_MS = 1000;
const MAX_BACKOFF_MS = 30000;
const BACKOFF_MULTIPLIER = 2;

/**
 * Hook that connects to the SSE endpoint for a case and dispatches events.
 * Implements exponential backoff reconnection (1s, 2s, 4s, 8s... max 30s).
 * Cleans up EventSource on unmount.
 */
export function useEventStream({ caseId, onEvent, enabled = true }: UseEventStreamOptions): ConnectionState {
  const eventSourceRef = useRef<EventSource | null>(null);
  const backoffRef = useRef(INITIAL_BACKOFF_MS);
  const reconnectTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const connectionStateRef = useRef<ConnectionState>('disconnected');
  const onEventRef = useRef(onEvent);
  const lastEventIdRef = useRef<string | null>(null);

  // Keep onEvent ref current to avoid stale closures
  onEventRef.current = onEvent;

  const cleanup = useCallback(() => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }
    if (eventSourceRef.current) {
      eventSourceRef.current.close();
      eventSourceRef.current = null;
    }
    connectionStateRef.current = 'disconnected';
  }, []);

  const connect = useCallback(() => {
    if (!caseId || !enabled) return;

    cleanup();
    connectionStateRef.current = 'connecting';

    // Build URL with auth token as query param (EventSource does not support custom headers)
    const url = `${API_BASE_URL}/api/cases/${encodeURIComponent(caseId)}/events/stream`;
    const eventSource = new EventSource(url);
    eventSourceRef.current = eventSource;

    eventSource.onopen = () => {
      connectionStateRef.current = 'connected';
      backoffRef.current = INITIAL_BACKOFF_MS;
    };

    eventSource.onerror = () => {
      connectionStateRef.current = 'disconnected';
      eventSource.close();
      eventSourceRef.current = null;

      // Schedule reconnection with exponential backoff
      const delay = backoffRef.current;
      backoffRef.current = Math.min(delay * BACKOFF_MULTIPLIER, MAX_BACKOFF_MS);

      reconnectTimerRef.current = setTimeout(() => {
        connect();
      }, delay);
    };

    // Register listeners for each known event type
    for (const eventType of SSE_EVENT_TYPES) {
      eventSource.addEventListener(eventType, (messageEvent: MessageEvent) => {
        try {
          const parsed = JSON.parse(messageEvent.data) as {
            type: SSEEventType;
            data: unknown;
            timestamp: string;
          };

          // Track last event ID for potential reconnection
          if (messageEvent.lastEventId) {
            lastEventIdRef.current = messageEvent.lastEventId;
          }

          const sseEvent: SSEEvent = {
            type: parsed.type ?? eventType,
            data: parsed.data,
            timestamp: parsed.timestamp ?? new Date().toISOString(),
          };

          onEventRef.current(sseEvent);
        } catch {
          // Silently ignore malformed events
        }
      });
    }
  }, [caseId, enabled, cleanup]);

  useEffect(() => {
    if (enabled && caseId) {
      connect();
    }

    return cleanup;
  }, [connect, cleanup, enabled, caseId]);

  return connectionStateRef.current;
}
