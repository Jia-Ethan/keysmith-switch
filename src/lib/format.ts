export function formatBytes(n: number | null | undefined): string {
  if (n == null || Number.isNaN(n) || n <= 0) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

export function shortPath(path: string, max = 56): string {
  if (path.length <= max) return path;
  const keep = Math.max(12, Math.floor((max - 1) / 2));
  return `${path.slice(0, keep)}…${path.slice(-keep)}`;
}

export function formatArgv(argv: string[] | null | undefined): string {
  if (!argv || argv.length === 0) return "—";
  return argv.join(" ");
}

/**
 * "3 minutes ago" style label in the UI language; older than a month falls
 * back to a plain date so the list never claims false precision.
 */
export function relativeTime(value: string | null | undefined, language: string, now = Date.now()): string {
  if (!value) return "";
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) return value.slice(0, 10);
  const seconds = Math.round((parsed.getTime() - now) / 1000);
  const abs = Math.abs(seconds);
  let rtf: Intl.RelativeTimeFormat;
  try {
    rtf = new Intl.RelativeTimeFormat(language, { numeric: "auto" });
  } catch {
    return parsed.toISOString().slice(0, 10);
  }
  if (abs < 45) return rtf.format(0, "second");
  if (abs < 3600) return rtf.format(Math.round(seconds / 60), "minute");
  if (abs < 86_400) return rtf.format(Math.round(seconds / 3600), "hour");
  if (abs < 30 * 86_400) return rtf.format(Math.round(seconds / 86_400), "day");
  return parsed.toISOString().slice(0, 10);
}

export function formatCount(value: number, language: string): string {
  try {
    return new Intl.NumberFormat(language).format(value);
  } catch {
    return String(value);
  }
}

/** The last part of a path, whichever way its separators lean. */
export function baseName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}
