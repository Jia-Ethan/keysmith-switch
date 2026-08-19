export function formatBytes(n: number | null | undefined): string {
  if (n == null || Number.isNaN(n)) return "—";
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
