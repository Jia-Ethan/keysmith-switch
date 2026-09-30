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

/** Eight-tooth gear with a hub; drawn for this set, not copied from an icon library. */
export function IconSettings(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M7.16 2.97 L7.33 1.43 L8.67 1.43 L8.84 2.97 L10.97 3.85 L12.17 2.88 L13.12 3.83 L12.15 5.03 L13.03 7.16 L14.57 7.33 L14.57 8.67 L13.03 8.84 L12.15 10.97 L13.12 12.17 L12.17 13.12 L10.97 12.15 L8.84 13.03 L8.67 14.57 L7.33 14.57 L7.16 13.03 L5.03 12.15 L3.83 13.12 L2.88 12.17 L3.85 10.97 L2.97 8.84 L1.43 8.67 L1.43 7.33 L2.97 7.16 L3.85 5.03 L2.88 3.83 L3.83 2.88 L5.03 3.85Z" />
      <circle cx="8" cy="8" r="2.1" />
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

export function IconUpload(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 10.4V2.9M5.2 5.6 8 2.8l2.8 2.8" />
      <path d="M2.9 10.2v2.3a.9.9 0 0 0 .9.9h8.4a.9.9 0 0 0 .9-.9v-2.3" />
    </Svg>
  );
}

export function IconArrowRight(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M3 8h10M9.2 4.2 13 8l-3.8 3.8" />
    </Svg>
  );
}

export function IconShield(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 1.9 3 3.8v3.9c0 3 2.1 5.3 5 6.4 2.9-1.1 5-3.4 5-6.4V3.8z" />
      <path d="M5.8 8.1 7.3 9.6l3-3.2" />
    </Svg>
  );
}

export function IconRocket(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9.6 2.3c2.1-.6 3.7-.4 4.1 0s.6 2-.1 4.1c-.6 1.8-2.4 3.7-4.7 5.1L6.5 9.1C7.9 6.8 7.8 2.9 9.6 2.3z" />
      <path d="M6.5 9.1 4.2 8.6l1.5-2.2 2.2-.2M9 11.5l.5 2.3 2.2-1.5.2-2.2" />
      <circle cx="10.6" cy="5.4" r="0.9" />
      <path d="M4.5 11.5c-1 .4-1.6 1.4-1.8 2.8 1.4-.2 2.4-.8 2.8-1.8" />
    </Svg>
  );
}

export function IconFile(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M9.2 1.9H4.4a1.2 1.2 0 0 0-1.2 1.2v9.8a1.2 1.2 0 0 0 1.2 1.2h7.2a1.2 1.2 0 0 0 1.2-1.2V5.5z" />
      <path d="M9.2 1.9v3.6h3.6" />
    </Svg>
  );
}

export function IconLibrary(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="2.2" y="2.6" width="3" height="10.8" rx="0.8" />
      <rect x="6.5" y="2.6" width="3" height="10.8" rx="0.8" />
      <path d="m10.3 3.3 2.5-.5 1.8 10.2-2.5.5z" />
    </Svg>
  );
}

export function IconSparkle(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M8 1.8c.4 2.9 1.3 3.8 4.2 4.2-2.9.4-3.8 1.3-4.2 4.2-.4-2.9-1.3-3.8-4.2-4.2 2.9-.4 3.8-1.3 4.2-4.2z" />
      <path d="M12.6 10.4c.2 1.3.6 1.7 1.9 1.9-1.3.2-1.7.6-1.9 1.9-.2-1.3-.6-1.7-1.9-1.9 1.3-.2 1.7-.6 1.9-1.9z" />
    </Svg>
  );
}

export function IconEye(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M1.6 8s2.3-4.6 6.4-4.6S14.4 8 14.4 8s-2.3 4.6-6.4 4.6S1.6 8 1.6 8z" />
      <circle cx="8" cy="8" r="1.9" />
    </Svg>
  );
}

export function IconSort(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M4.8 2.8v10.4M2.6 5 4.8 2.8 7 5M11.2 13.2V2.8M9 11l2.2 2.2 2.2-2.2" />
    </Svg>
  );
}

export function IconTerminal(props: IconProps) {
  return (
    <Svg {...props}>
      <rect x="1.9" y="2.9" width="12.2" height="10.2" rx="2.2" />
      <path d="m4.9 6.4 2 1.6-2 1.6M8.6 9.9h2.6" />
    </Svg>
  );
}

/** A jigsaw piece: the extensions entry. */
export function IconPuzzle(props: IconProps) {
  return (
    <Svg {...props}>
      <path d="M6.2 2.6a1.5 1.5 0 0 1 3 0v.9h2.3c.5 0 .9.4.9.9v2.2h.9a1.5 1.5 0 0 1 0 3h-.9v2.4c0 .5-.4.9-.9.9H9.2v-.9a1.5 1.5 0 0 0-3 0v.9H3.9c-.5 0-.9-.4-.9-.9V9.5h.9a1.5 1.5 0 0 0 0-3H3V4.4c0-.5.4-.9.9-.9h2.3z" />
    </Svg>
  );
}
