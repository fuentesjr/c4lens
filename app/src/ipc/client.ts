import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import type { CodeRef, EffectiveModel, RepoHandle, ScanSummary, ValidationReport } from "../model/types";

export interface OpenRepoResult {
  repo: RepoHandle;
  model: EffectiveModel | null;
}

export interface ModelChangedPayload {
  repoId: string;
  sourceSha: string;
  validation: ValidationReport;
}

export interface ValidationFailedPayload {
  repoId: string;
  validation: ValidationReport;
}

export interface IndexUpdatedPayload {
  repoId: string;
  summary: ScanSummary;
}

export interface ModelEventHandlers {
  onModelChanged: (payload: ModelChangedPayload) => void | Promise<void>;
  onValidationFailed: (payload: ValidationFailedPayload) => void | Promise<void>;
  onIndexUpdated?: (payload: IndexUpdatedPayload) => void | Promise<void>;
}

export function isTauriDesktop(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export async function openRepositoryFromDialog(): Promise<OpenRepoResult | null> {
  if (!isTauriDesktop()) {
    return null;
  }

  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open repository",
  });

  if (typeof selected !== "string") {
    return null;
  }

  const repo = await invoke<RepoHandle>("open_repo", { path: selected });
  let model: EffectiveModel | null = null;
  try {
    model = await invoke<EffectiveModel>("get_model");
  } catch {
    model = null;
  }
  return { repo, model };
}

export async function fetchActiveModel(): Promise<EffectiveModel | null> {
  if (!isTauriDesktop()) {
    return null;
  }

  try {
    return await invoke<EffectiveModel>("get_model");
  } catch {
    return null;
  }
}

export async function scanCodebase(params: { force?: boolean } = {}): Promise<ScanSummary> {
  if (!isTauriDesktop()) {
    throw new Error("Scan is available in the Tauri desktop shell");
  }

  return await invoke<ScanSummary>("scan_codebase", { params });
}

export async function getElementCode(slug: string): Promise<CodeRef | null> {
  if (!isTauriDesktop()) {
    return null;
  }

  return await invoke<CodeRef | null>("get_element_code", { params: { slug } });
}

export async function openInEditor(path: string, line?: number, column?: number): Promise<void> {
  if (!isTauriDesktop()) {
    return;
  }

  await invoke("open_in_editor", { params: { path, line, column } });
}

export async function listenToModelEvents(handlers: ModelEventHandlers): Promise<UnlistenFn> {
  if (!isTauriDesktop()) {
    return () => {};
  }

  const unlistenModelChanged = await listen<ModelChangedPayload>("model-changed", (event) => {
    void handlers.onModelChanged(event.payload);
  });
  const unlistenValidationFailed = await listen<ValidationFailedPayload>("validation-failed", (event) => {
    void handlers.onValidationFailed(event.payload);
  });
  const unlistenIndexUpdated = await listen<IndexUpdatedPayload>("index-updated", (event) => {
    void handlers.onIndexUpdated?.(event.payload);
  });

  return () => {
    unlistenModelChanged();
    unlistenValidationFailed();
    unlistenIndexUpdated();
  };
}
