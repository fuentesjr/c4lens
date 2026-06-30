import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { EffectiveModel, RepoHandle } from "../model/types";

export interface OpenRepoResult {
  repo: RepoHandle;
  model: EffectiveModel;
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
  const model = await invoke<EffectiveModel>("get_model");
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
