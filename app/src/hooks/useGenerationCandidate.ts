import { useCallback, useState, type MutableRefObject } from "react";

import { applyGenerated, generateModel, isTauriDesktop } from "../ipc/client";
import type { GenerationDiff, GenerationSummary } from "../model/types";

interface UseGenerationCandidateOptions {
  activeRepoIdRef: MutableRefObject<string>;
  setStatus: (status: string) => void;
  formatError: (error: unknown, fallback: string) => string;
  formatSummary: (summary: GenerationSummary) => string;
}

export function useGenerationCandidate({
  activeRepoIdRef,
  setStatus,
  formatError,
  formatSummary,
}: UseGenerationCandidateOptions) {
  const [candidate, setCandidate] = useState<GenerationDiff | null>(null);
  const [isGenerating, setIsGenerating] = useState(false);
  const [isApplying, setIsApplying] = useState(false);

  const clearCandidate = useCallback(() => {
    setCandidate(null);
  }, []);

  const runGenerate = useCallback(async () => {
    if (!isTauriDesktop()) {
      setStatus("Generation is available in the Tauri desktop shell");
      return;
    }

    setIsGenerating(true);
    setCandidate(null);
    setStatus("Scanning and generating model");
    const generationRepoId = activeRepoIdRef.current;
    try {
      const nextCandidate = await generateModel({ scanFirst: true });
      if (activeRepoIdRef.current === generationRepoId && nextCandidate.repo.id === generationRepoId) {
        setCandidate(nextCandidate);
        setStatus(`Generated ${formatSummary(nextCandidate.summary)}`);
      }
    } catch (error) {
      if (activeRepoIdRef.current === generationRepoId) {
        setStatus(formatError(error, "Generation failed"));
      }
    } finally {
      setIsGenerating(false);
    }
  }, [activeRepoIdRef, formatError, formatSummary, setStatus]);

  const applyCandidate = useCallback(async () => {
    if (!candidate) {
      return;
    }
    if (!isTauriDesktop()) {
      setStatus("Apply is available in the Tauri desktop shell");
      return;
    }

    setIsApplying(true);
    setStatus("Applying generated model");
    const generationRepoId = candidate.repo.id;
    try {
      await applyGenerated({
        generationId: candidate.candidateId,
        expectedAuthoredSha: candidate.baseAuthoredSha,
        expectedOverlaySha: candidate.baseOverlaySha,
        expectedModelSourceSha: candidate.modelSourceSha,
        expectedIndexScanToken: candidate.indexScanToken,
        expectedSchemaVersion: candidate.schemaVersion,
        selection: { acceptAll: true },
      });
      if (activeRepoIdRef.current === generationRepoId) {
        setCandidate(null);
        setStatus("Applied generated model");
      }
    } catch (error) {
      if (activeRepoIdRef.current === generationRepoId) {
        setStatus(formatError(error, "Apply failed"));
      }
    } finally {
      setIsApplying(false);
    }
  }, [activeRepoIdRef, candidate, formatError, setStatus]);

  return {
    candidate,
    isApplying,
    isGenerating,
    clearCandidate,
    runGenerate,
    applyCandidate,
  };
}
