import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { ImportCandidate } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { Checkbox, Mono } from "./ui";

export function FirstRunDialog({
  open,
  candidates,
  onImport,
  onSkip,
}: {
  open: boolean;
  candidates: ImportCandidate[];
  onImport: (paths: string[]) => void;
  onSkip: () => void;
}) {
  const { t } = useTranslation();
  const [selected, setSelected] = useState<Record<string, boolean>>({});
  const paths = candidates.filter((item) => selected[item.id]).map((item) => item.path);

  return (
    <ConfirmDialog
      open={open}
      title={t("firstRun.title")}
      description={t("firstRun.hint")}
      confirmLabel={t("firstRun.import")}
      cancelLabel={t("firstRun.skip")}
      confirmDisabled={paths.length === 0}
      onClose={onSkip}
      onConfirm={() => onImport(paths)}
    >
      <div className="flex flex-col gap-2">
        {candidates.map((item) => (
          <Checkbox
            key={item.id}
            label={
              <span>
                {item.title} · {item.tool}
                <Mono className="ml-2">{item.path}</Mono>
              </span>
            }
            checked={Boolean(selected[item.id])}
            onChange={(event) =>
              setSelected((current) => ({ ...current, [item.id]: event.target.checked }))
            }
          />
        ))}
      </div>
    </ConfirmDialog>
  );
}
