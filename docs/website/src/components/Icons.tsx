import React from "react";

interface IconProps {
  size?: number;
  className?: string;
}

const defaultProps = { size: 24, className: "" };

export function TemporalGraphIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <circle cx="6" cy="6" r="2.5" />
      <circle cx="18" cy="6" r="2.5" />
      <circle cx="12" cy="18" r="2.5" />
      <line x1="8.2" y1="7" x2="10" y2="16" />
      <line x1="15.8" y1="7" x2="14" y2="16" />
      <line x1="8.5" y1="6" x2="15.5" y2="6" />
      <path d="M21 12l1.5 1.5L21 15" opacity="0.5" />
      <line x1="19" y1="13.5" x2="22.5" y2="13.5" opacity="0.5" />
    </svg>
  );
}

export function HybridSearchIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <circle cx="10" cy="10" r="6" />
      <line x1="14.5" y1="14.5" x2="20" y2="20" />
      <path d="M7 10h6" />
      <path d="M10 7v6" />
      <circle cx="10" cy="10" r="3" opacity="0.3" strokeDasharray="2 2" />
    </svg>
  );
}

export function McpNativeIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <rect x="3" y="3" width="7" height="7" rx="1.5" />
      <rect x="14" y="3" width="7" height="7" rx="1.5" />
      <rect x="3" y="14" width="7" height="7" rx="1.5" />
      <rect x="14" y="14" width="7" height="7" rx="1.5" />
      <line x1="10" y1="6.5" x2="14" y2="6.5" />
      <line x1="6.5" y1="10" x2="6.5" y2="14" />
      <line x1="17.5" y1="10" x2="17.5" y2="14" />
      <line x1="10" y1="17.5" x2="14" y2="17.5" />
    </svg>
  );
}

export function TraitArchIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M12 2L2 7l10 5 10-5-10-5z" />
      <path d="M2 17l10 5 10-5" />
      <path d="M2 12l10 5 10-5" />
    </svg>
  );
}

export function SurrealDbIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <ellipse cx="12" cy="5" rx="8" ry="3" />
      <path d="M4 5v6c0 1.66 3.58 3 8 3s8-1.34 8-3V5" />
      <path d="M4 11v6c0 1.66 3.58 3 8 3s8-1.34 8-3v-6" />
    </svg>
  );
}

export function ShipAnywhereIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M5 18h14a2 2 0 002-2V8a2 2 0 00-2-2H5a2 2 0 00-2 2v8a2 2 0 002 2z" />
      <path d="M15 6V4a2 2 0 00-2-2h-2a2 2 0 00-2 2v2" />
      <line x1="12" y1="10" x2="12" y2="14" />
      <line x1="10" y1="12" x2="14" y2="12" />
      <line x1="3" y1="22" x2="21" y2="22" opacity="0.4" strokeDasharray="2 3" />
    </svg>
  );
}

export function ConversationIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
      <circle cx="9" cy="10" r="1" fill="currentColor" stroke="none" />
      <circle cx="12" cy="10" r="1" fill="currentColor" stroke="none" />
      <circle cx="15" cy="10" r="1" fill="currentColor" stroke="none" />
    </svg>
  );
}

export function KnowledgeBaseIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <path d="M4 19.5A2.5 2.5 0 016.5 17H20" />
      <path d="M6.5 2H20v20H6.5A2.5 2.5 0 014 19.5v-15A2.5 2.5 0 016.5 2z" />
      <circle cx="12" cy="10" r="3" opacity="0.5" />
      <line x1="12" y1="13" x2="12" y2="15" opacity="0.5" />
    </svg>
  );
}

export function CodebaseIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <polyline points="16 18 22 12 16 6" />
      <polyline points="8 6 2 12 8 18" />
      <line x1="14" y1="4" x2="10" y2="20" opacity="0.4" />
    </svg>
  );
}

export function TemporalAuditIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <circle cx="12" cy="12" r="9" />
      <polyline points="12 7 12 12 15 15" />
      <path d="M16.5 3.5L18 2" opacity="0.5" />
      <path d="M7.5 3.5L6 2" opacity="0.5" />
    </svg>
  );
}

export function MultiAgentIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <circle cx="7" cy="6" r="3" />
      <circle cx="17" cy="6" r="3" />
      <circle cx="12" cy="17" r="3" />
      <line x1="9" y1="8" x2="11" y2="14.5" strokeDasharray="2 2" opacity="0.4" />
      <line x1="15" y1="8" x2="13" y2="14.5" strokeDasharray="2 2" opacity="0.4" />
      <line x1="9.5" y1="6" x2="14.5" y2="6" strokeDasharray="2 2" opacity="0.4" />
    </svg>
  );
}

export function PersonalGraphIcon({ size = defaultProps.size, className }: IconProps) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" className={className}>
      <circle cx="12" cy="8" r="4" />
      <path d="M5 20c0-3.87 3.13-7 7-7s7 3.13 7 7" />
      <circle cx="18" cy="14" r="2" opacity="0.5" />
      <line x1="18" y1="14" x2="15" y2="11" opacity="0.3" strokeDasharray="1 2" />
    </svg>
  );
}

export const featureIcons = {
  temporal: TemporalGraphIcon,
  search: HybridSearchIcon,
  mcp: McpNativeIcon,
  traits: TraitArchIcon,
  surreal: SurrealDbIcon,
  ship: ShipAnywhereIcon,
};

export const useCaseIcons = {
  conversation: ConversationIcon,
  knowledge: KnowledgeBaseIcon,
  codebase: CodebaseIcon,
  audit: TemporalAuditIcon,
  multiAgent: MultiAgentIcon,
  personal: PersonalGraphIcon,
};
