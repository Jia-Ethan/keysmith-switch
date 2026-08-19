import type { SVGProps } from "react";

/**
 * Minimal inline icon set. Stroke-based, 16px grid, currentColor.
 * Kept local on purpose: no extra icon dependency is added to the bundle.
 */
type IconProps = SVGProps<SVGSVGElement> & { size?: number };

function Svg({ size = 14, children, ...props }: IconProps) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 16 16"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.5}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...props}
    >
      {children}
    </svg>
  );
}

export function IconSearch(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="7" cy="7" r="4.25" />
      <path d="M10.2 10.2 13.5 13.5" />
    </Svg>
  );
}

export function IconClose(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.5 3.5 12.5 12.5M12.5 3.5 3.5 12.5" />
    </Svg>
  );
}

export function IconSettings(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="8" cy="8" r="2.1" />
      <path d="M8 1.6v1.6M8 12.8v1.6M3.5 3.5l1.15 1.15M11.35 11.35l1.15 1.15M1.6 8h1.6M12.8 8h1.6M3.5 12.5l1.15-1.15M11.35 4.65 12.5 3.5" />
    </Svg>
  );
}

export function IconInfo(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="8" cy="8" r="6.4" />
      <path d="M8 7.2v4M8 4.9h.01" />
    </Svg>
  );
}

export function IconRefresh(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M13.4 8a5.4 5.4 0 1 1-1.7-3.9" />
      <path d="M13.6 2.2v2.9h-2.9" />
    </Svg>
  );
}

export function IconPlus(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 3.2v9.6M3.2 8h9.6" />
    </Svg>
  );
}

export function IconPencil(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M11.1 2.6a1.4 1.4 0 0 1 2 2L5.6 12.1l-2.8.7.7-2.8z" />
    </Svg>
  );
}

export function IconTrash(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M2.8 4.4h10.4M6.2 4.4V3.1h3.6v1.3M4.2 4.4l.6 8.2h6.4l.6-8.2" />
    </Svg>
  );
}

export function IconCopy(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="5.6" y="5.6" width="7.6" height="7.6" rx="1.4" />
      <path d="M10.4 5.6V4.2a1.4 1.4 0 0 0-1.4-1.4H4.2a1.4 1.4 0 0 0-1.4 1.4v4.8a1.4 1.4 0 0 0 1.4 1.4h1.4" />
    </Svg>
  );
}

export function IconChevronDown(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4 6.2 8 10.2l4-4" />
    </Svg>
  );
}

export function IconChevronRight(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M6.2 4 10.2 8l-4 4" />
    </Svg>
  );
}

export function IconMore(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="3.4" cy="8" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="8" cy="8" r="0.9" fill="currentColor" stroke="none" />
      <circle cx="12.6" cy="8" r="0.9" fill="currentColor" stroke="none" />
    </Svg>
  );
}

export function IconSun(props: IconProps) {
  return (
    <Svg {...props}>
      <circle cx="8" cy="8" r="2.8" />
      <path d="M8 1.4v1.5M8 13.1v1.5M1.4 8h1.5M13.1 8h1.5M3.3 3.3l1.1 1.1M11.6 11.6l1.1 1.1M3.3 12.7l1.1-1.1M11.6 4.4l1.1-1.1" />
    </Svg>
  );
}

export function IconMoon(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M13 9.8A5.6 5.6 0 0 1 6.2 3a5.6 5.6 0 1 0 6.8 6.8z" />
    </Svg>
  );
}

export function IconMonitor(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="1.9" y="2.9" width="12.2" height="8" rx="1.2" />
      <path d="M5.6 13.4h4.8M8 10.9v2.5" />
    </Svg>
  );
}

export function IconFolder(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M1.9 4.6a1.2 1.2 0 0 1 1.2-1.2h2.5l1.3 1.5h5.2a1.2 1.2 0 0 1 1.2 1.2v5.3a1.2 1.2 0 0 1-1.2 1.2H3.1a1.2 1.2 0 0 1-1.2-1.2z" />
    </Svg>
  );
}

export function IconAlert(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M7 2.6 1.9 11.6a1.1 1.1 0 0 0 1 1.7h10.2a1.1 1.1 0 0 0 1-1.7L9 2.6a1.1 1.1 0 0 0-2 0z" />
      <path d="M8 6.2v3M8 11.1h.01" />
    </Svg>
  );
}

export function IconCheck(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3.2 8.4 6.4 11.6l6.4-7.2" />
    </Svg>
  );
}

export function IconHistory(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M2.6 8a5.4 5.4 0 1 0 1.7-3.9" />
      <path d="M2.4 2.2v2.9h2.9" />
      <path d="M8 5.2V8l2.1 1.6" />
    </Svg>
  );
}

export function IconDownload(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 2.6v6.6M5.2 6.6 8 9.4l2.8-2.8" />
      <path d="M2.9 11.4v1.1a.9.9 0 0 0 .9.9h8.4a.9.9 0 0 0 .9-.9v-1.1" />
    </Svg>
  );
}

export function IconExternal(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9.4 2.6h4v4M13.4 2.6 7.6 8.4" />
      <path d="M12.2 9.6v2.6a1.2 1.2 0 0 1-1.2 1.2H3.8a1.2 1.2 0 0 1-1.2-1.2V5a1.2 1.2 0 0 1 1.2-1.2h2.6" />
    </Svg>
  );
}

export function IconPower(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 2.4v4.4" />
      <path d="M4.6 4.4a4.8 4.8 0 1 0 6.8 0" />
    </Svg>
  );
}
