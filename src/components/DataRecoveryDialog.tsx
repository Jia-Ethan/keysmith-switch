import { useTranslation } from "react-i18next";
import type { RecoveryMarker } from "../types";
import { ConfirmDialog } from "./ConfirmDialog";
import { Mono } from "./ui";

export function DataRecoveryDialog({
  marker,
  onAck,
}: {
  marker: RecoveryMarker | null;
  onAck: () => void;
}) {
  const { t } = useTranslation();
  return (
    <ConfirmDialog
      open={Boolean(marker)}
      title={t("recovery.title")}
      description={t("recovery.hint")}
      confirmLabel={t("common.ok")}
      cancelLabel={t("common.close")}
      closeLabel={t("common.close")}
      onClose={onAck}
      onConfirm={onAck}
    >
      {marker ? (
        <div className="space-y-2 text-sm">
          <p>{marker.kind}</p>
          {marker.quarantined ? <Mono>{marker.quarantined}</Mono> : null}
          <p>{marker.detail}</p>
        </div>
      ) : null}
    </ConfirmDialog>
  );
}
