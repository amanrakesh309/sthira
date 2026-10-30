import { useState, useCallback, useRef } from 'react';
import { streamAdaptiveRun } from '../api/ipc';
import type {
  ComparisonRow,
  DecisionPoint,
  OperationalMode,
  PlanPhase,
  ProgressiveStreamChunk,
  StreamMetrics,
  StreamStatus,
} from '../types/sthira';

export interface UseAdaptiveStreamReturn {
  rawText: string;
  phases: PlanPhase[];
  decisions: DecisionPoint[];
  comparisonRows: ComparisonRow[];
  status: StreamStatus;
  viewMode: 'widgets' | 'markdown';
  metrics: StreamMetrics | null;
  fallbackNotices: string[];
  error: string | null;
  basisFingerprint: string | null;
  hasStructuredWidgets: boolean;
  startStream: (
    rawInput: string,
    mode: OperationalMode,
    modelId?: string,
    onChunkReceived?: (chunk: ProgressiveStreamChunk) => void
  ) => Promise<string>;
  resetStream: () => void;
  setViewMode: (mode: 'widgets' | 'markdown') => void;
  toggleViewMode: () => void;
}

export function useAdaptiveStream(): UseAdaptiveStreamReturn {
  const [rawText, setRawText] = useState('');
  const [phases, setPhases] = useState<PlanPhase[]>([]);
  const [decisions, setDecisions] = useState<DecisionPoint[]>([]);
  const [comparisonRows, setComparisonRows] = useState<ComparisonRow[]>([]);
  const [status, setStatus] = useState<StreamStatus>('idle');
  const [viewMode, setViewMode] = useState<'widgets' | 'markdown'>('widgets');
  const [metrics, setMetrics] = useState<StreamMetrics | null>(null);
  const [fallbackNotices, setFallbackNotices] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [basisFingerprint, setBasisFingerprint] = useState<string | null>(null);

  const isStreamingRef = useRef(false);

  const resetStream = useCallback(() => {
    setRawText('');
    setPhases([]);
    setDecisions([]);
    setComparisonRows([]);
    setStatus('idle');
    setViewMode('widgets');
    setMetrics(null);
    setFallbackNotices([]);
    setError(null);
    setBasisFingerprint(null);
    isStreamingRef.current = false;
  }, []);

  const toggleViewMode = useCallback(() => {
    setViewMode((prev) => (prev === 'widgets' ? 'markdown' : 'widgets'));
  }, []);

  const handleChunk = useCallback((chunk: ProgressiveStreamChunk) => {
    switch (chunk.type) {
      case 'text_token':
        setRawText((prev) => prev + chunk.payload);
        break;

      case 'plan_phase':
        setPhases((prev) => {
          // Deduplicate by phase_number
          const existingIdx = prev.findIndex((p) => p.phase_number === chunk.payload.phase_number);
          if (existingIdx >= 0) {
            const next = [...prev];
            next[existingIdx] = chunk.payload;
            return next;
          }
          return [...prev, chunk.payload];
        });
        break;

      case 'decision_point':
        setDecisions((prev) => {
          const existingIdx = prev.findIndex((d) => d.id === chunk.payload.id);
          if (existingIdx >= 0) {
            const next = [...prev];
            next[existingIdx] = chunk.payload;
            return next;
          }
          return [...prev, chunk.payload];
        });
        break;

      case 'comparison_row':
        setComparisonRows((prev) => {
          const existingIdx = prev.findIndex((r) => r.criteria === chunk.payload.criteria);
          if (existingIdx >= 0) {
            const next = [...prev];
            next[existingIdx] = chunk.payload;
            return next;
          }
          return [...prev, chunk.payload];
        });
        break;

      case 'complete':
        setMetrics(chunk.payload);
        setBasisFingerprint(chunk.payload.basis_fingerprint);
        setStatus('completed');
        isStreamingRef.current = false;
        break;

      case 'fallback_notice':
        setFallbackNotices((prev) => [...prev, chunk.payload.reason]);
        break;

      default:
        break;
    }
  }, []);

  const startStream = useCallback(
    async (
      rawInput: string,
      mode: OperationalMode,
      modelId?: string,
      onChunkReceived?: (chunk: ProgressiveStreamChunk) => void
    ): Promise<string> => {
      resetStream();
      setStatus('streaming');
      isStreamingRef.current = true;
      let accumulated = '';

      try {
        await streamAdaptiveRun(rawInput, mode, modelId, (chunk) => {
          handleChunk(chunk);
          if (chunk.type === 'text_token') {
            accumulated += chunk.payload;
          }
          onChunkReceived?.(chunk);
        });
        // In case complete chunk was not sent
        setStatus((cur) => (cur === 'streaming' ? 'completed' : cur));
        return accumulated;
      } catch (err) {
        console.error('Adaptive stream execution error:', err);
        setError(String(err));
        setStatus('degraded_to_markdown');
        return accumulated;
      } finally {
        isStreamingRef.current = false;
      }
    },
    [handleChunk, resetStream]
  );

  const hasStructuredWidgets =
    phases.length > 0 || decisions.length > 0 || comparisonRows.length > 0;

  return {
    rawText,
    phases,
    decisions,
    comparisonRows,
    status,
    viewMode,
    metrics,
    fallbackNotices,
    error,
    basisFingerprint,
    hasStructuredWidgets,
    startStream,
    resetStream,
    setViewMode,
    toggleViewMode,
  };
}
