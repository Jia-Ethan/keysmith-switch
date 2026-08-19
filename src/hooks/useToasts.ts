import { useCallback, useMemo, useState } from "react";
import { toastSafeMessage } from "../lib/redact";

export type ToastKind = "info" | "ok" | "err";

export interface ToastItem {
  id: string;
  kind: ToastKind;
  message: string;
}

let nextId = 1;

export function useToasts() {
  const [toasts, setToasts] = useState<ToastItem[]>([]);

  const dismiss = useCallback((id: string) => {
    setToasts((current) => current.filter((item) => item.id !== id));
  }, []);

  const push = useCallback(
    (kind: ToastKind, input: unknown) => {
      const message = toastSafeMessage(input);
      if (!message) return;
      const id = `t${nextId++}`;
      setToasts((current) => [...current.slice(-4), { id, kind, message }]);
      window.setTimeout(() => dismiss(id), 4200);
    },
    [dismiss],
  );

  return useMemo(
    () => ({
      toasts,
      dismiss,
      info: (input: unknown) => push("info", input),
      ok: (input: unknown) => push("ok", input),
      err: (input: unknown) => push("err", input),
    }),
    [toasts, dismiss, push],
  );
}

export type ToastApi = ReturnType<typeof useToasts>;
