// Mise à jour SIGNÉE (updater Tauri). Câblage complet ; INERTE tant qu'une release
// signée + une vraie clé publique minisign ne sont pas en place (voir docs/16 —
// activation gated sur le secret TAURI_SIGNING_PRIVATE_KEY). En navigateur (mock),
// l'updater n'existe pas → « non disponible ».
import { isMock } from "./ipc";

export type UpdateStatus =
  | { kind: "unsupported" } // navigateur / build sans runtime Tauri
  | { kind: "up_to_date" }
  | { kind: "available"; version: string; notes: string | null }
  | { kind: "error"; message: string };

/// Interroge l'endpoint de mise à jour. Ne télécharge rien. Sans release signée
/// (ou hors Tauri), renvoie `up_to_date`/`unsupported` plutôt qu'une erreur.
export async function checkForUpdate(): Promise<UpdateStatus> {
  if (isMock) return { kind: "unsupported" };
  try {
    const { check } = await import("@tauri-apps/plugin-updater");
    const update = await check();
    if (!update) return { kind: "up_to_date" };
    return { kind: "available", version: update.version, notes: update.body ?? null };
  } catch (e) {
    return { kind: "error", message: e instanceof Error ? e.message : String(e) };
  }
}

/// Télécharge et installe la mise à jour disponible (signature vérifiée par le
/// plugin contre la clé publique). L'application doit être RELANCÉE pour appliquer
/// (l'ajout de `tauri-plugin-process` permettrait un relaunch automatique).
export async function installUpdate(): Promise<void> {
  if (isMock) return;
  const { check } = await import("@tauri-apps/plugin-updater");
  const update = await check();
  if (!update) return;
  await update.downloadAndInstall();
}
