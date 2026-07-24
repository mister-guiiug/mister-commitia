// Suivi des opérations longues côté UI (T2/T11) : une tâche à la fois par
// page — progression, annulation coopérative, fragments IA streamés.

import { useEffect, useRef, useState } from "react";
import { cancelTask, newTaskId, onTaskEvent } from "./ipc";

export interface RunningTask {
  id: string;
  label: string;
  phase: string;
  current: number;
  total: number | null;
}

export function useTask(onAiDelta?: (group: number, delta: string) => void) {
  const [running, setRunning] = useState<RunningTask | null>(null);
  const idRef = useRef<string | null>(null);
  const deltaRef = useRef(onAiDelta);
  deltaRef.current = onAiDelta;

  useEffect(() => {
    let unsubscribe: (() => void) | undefined;
    let disposed = false;
    void onTaskEvent((e) => {
      if (e.task_id !== idRef.current) return;
      if (e.kind === "progress") {
        setRunning((r) =>
          r && r.id === e.task_id
            ? { ...r, phase: e.phase, current: e.current, total: e.total }
            : r,
        );
      } else {
        deltaRef.current?.(e.group, e.delta);
      }
    }).then((u) => {
      if (disposed) u();
      else unsubscribe = u;
    });
    return () => {
      disposed = true;
      unsubscribe?.();
    };
  }, []);

  /// Démarre le suivi et retourne le taskId à joindre à l'appel IPC.
  const begin = (label: string): string => {
    const id = newTaskId();
    idRef.current = id;
    setRunning({ id, label, phase: "démarrage…", current: 0, total: null });
    return id;
  };

  const end = () => {
    idRef.current = null;
    setRunning(null);
  };

  const cancel = () => {
    if (idRef.current) cancelTask(idRef.current);
  };

  return { running, begin, end, cancel };
}
