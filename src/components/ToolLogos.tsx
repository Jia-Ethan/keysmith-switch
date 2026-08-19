import type { ToolId } from "../types";

/** Distinctive compatibility marks. Not official trademark assets. */
export function ToolLogo({ tool, size = 20 }: { tool: ToolId; size?: number }) {
  switch (tool) {
    case "claude":
      return <ClaudeMark size={size} />;
    case "codex":
      return <CodexMark size={size} />;
    case "grok":
      return <GrokMark size={size} />;
    case "zcode":
      return <ZcodeMark size={size} />;
  }
}

function ClaudeMark({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" className="shrink-0">
      <path
        fill="#D97706"
        d="M12 2.2 13.6 8.4 19.8 10 13.6 11.6 12 17.8 10.4 11.6 4.2 10 10.4 8.4z"
      />
      <circle cx="12" cy="10" r="2.1" fill="#F59E0B" />
    </svg>
  );
}

function CodexMark({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" className="shrink-0">
      <path
        fill="#2563EB"
        d="M12 3c.7 2.4 2.2 4.4 4.4 5.6C14.2 9.8 12.7 11.8 12 14.2 11.3 11.8 9.8 9.8 7.6 8.6 9.8 7.4 11.3 5.4 12 3z"
      />
      <path
        fill="#60A5FA"
        d="M12 10.2c.5 1.7 1.6 3.1 3.2 4-1.6.9-2.7 2.3-3.2 4-.5-1.7-1.6-3.1-3.2-4 1.6-.9 2.7-2.3 3.2-4z"
      />
    </svg>
  );
}

function GrokMark({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" className="shrink-0">
      <rect x="3" y="3" width="18" height="18" rx="4" fill="#111827" />
      <path stroke="#F97316" strokeWidth="2.2" strokeLinecap="round" d="M8 8 16 16M16 8 8 16" />
    </svg>
  );
}

function ZcodeMark({ size }: { size: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden="true" className="shrink-0">
      <rect x="3" y="3" width="18" height="18" rx="4" fill="#6D28D9" />
      <path
        fill="#F5F3FF"
        d="M7.5 7.2h9l-9 9.6h9v1.8h-10.8l9-9.6H7.5z"
      />
    </svg>
  );
}
