const MAX_TOAST_CHARS = 240;

const NAMED_SECRET =
  /((?:api[_-]?key|access[_-]?token|refresh[_-]?token|id[_-]?token|session[_-]?token|authorization|password|passwd|pwd|secret|cookie|credential|bearer)\s*["']?\s*[:=]\s*)(["']?)([^\s"',}]+)\2/gi;

const AUTH_SCHEME =
  /\b(Bearer|Basic|Token|ApiKey|Digest)\s+[^\s"',}\]]+/gi;

const URL_CREDENTIALS = /(https?:\/\/)[^/@\s]+@/gi;

const COOKIE_HEADER =
  /(^|[\r\n])([ \t]*(?:set-)?cookie\s*[:=]\s*)[^\r\n]+/gim;

const OPAQUE_SECRET =
  /\b(?:sk-[A-Za-z0-9._~+/-]{8,}|github_pat_[A-Za-z0-9_]{8,}|gh[pousr]_[A-Za-z0-9_]{8,}|xox[baprs]-[A-Za-z0-9-]{8,}|eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+)\b/g;

export function redactSecrets(input: unknown): string {
  const text = stringifyUnknown(input);
  return text
    .replace(URL_CREDENTIALS, "$1[REDACTED]@")
    .replace(COOKIE_HEADER, "$1$2[REDACTED]")
    .replace(AUTH_SCHEME, "$1 [REDACTED]")
    .replace(NAMED_SECRET, "$1$2[REDACTED]$2")
    .replace(OPAQUE_SECRET, "[REDACTED]");
}

export function toastSafeMessage(input: unknown): string {
  const redacted = redactSecrets(input).replace(/\s+/g, " ").trim();
  if (!redacted) return "";
  if (redacted.length <= MAX_TOAST_CHARS) return redacted;
  return `${redacted.slice(0, MAX_TOAST_CHARS)}…`;
}

function stringifyUnknown(input: unknown): string {
  if (input == null) return "";
  if (typeof input === "string") return input;
  if (input instanceof Error) return input.message;
  try {
    return JSON.stringify(input);
  } catch {
    return String(input);
  }
}
