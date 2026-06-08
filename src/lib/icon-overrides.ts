const ICONS_KEY = "nyxedit-ext-icons";

export function getExtensionIcons(): Record<string, string> {
  try {
    return JSON.parse(localStorage.getItem(ICONS_KEY) || "{}");
  } catch {
    return {};
  }
}

export function setExtensionIcons(icons: Record<string, string>) {
  try {
    localStorage.setItem(ICONS_KEY, JSON.stringify(icons));
  } catch {}
}

export function getIconForExt(ext: string, fallback: string): string {
  const icons = getExtensionIcons();
  return icons[ext] || fallback;
}

const FILE_COLORS: Record<string, string> = {
  ts: "#3178c6", js: "#f7df1e", rs: "#ff8243", py: "#3572A5",
  svelte: "#ff3e00", html: "#e34f26", css: "#38bdf8", json: "#10b981",
  md: "#a855f7", yml: "#fb7185", yaml: "#fb7185", toml: "#d946ef",
  sh: "#4ade80", bash: "#4ade80", ps1: "#38bdf8",
  png: "#fbbf24", jpg: "#fbbf24", jpeg: "#fbbf24",
  svg: "#fbbf24", gif: "#fbbf24",
  c: "#3b82f6", cpp: "#3b82f6", cxx: "#3b82f6", h: "#8b5cf6", hpp: "#8b5cf6", ino: "#00979d",
};

export function getColorForExt(ext: string): string {
  return FILE_COLORS[ext] || "var(--text-muted)";
}

export function getDefaultIconSvg(ext: string, name: string): { svg: string; color: string } {
  const color = getColorForExt(ext);
  let svg = "";

  if (ext === "svelte") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="10" fill="${color}" fill-opacity="0.1" stroke="${color}" stroke-width="1.5"/><path d="M12 6c-3.31 0-6 2.69-6 6s2.69 6 6 6 6-2.69 6-6-2.69-6-6-6zm0 10c-2.21 0-4-1.79-4-4s1.79-4 4-4 4 1.79 4 4-1.79 4-4 4z" fill="${color}" fill-opacity="0.6"/></svg>`;
  } else if (ext === "rs") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="7" stroke="${color}" stroke-width="1.5" stroke-dasharray="3 1.5"/><circle cx="12" cy="12" r="4.5" fill="${color}" fill-opacity="0.4" stroke="${color}" stroke-width="1"/></svg>`;
  } else if (ext === "json" || ext === "lock") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="1.5"><path d="M8 4H6a2 2 0 0 0-2 2v3a2 2 0 0 1-2 2 2 2 0 0 1 2 2v3a2 2 0 0 0 2 2h2M16 4h2a2 2 0 0 1 2 2v3a2 2 0 0 0 2 2 2 2 0 0 0-2 2v3a2 2 0 0 1-2 2h-2" stroke-linecap="round"/></svg>`;
  } else if (ext === "js") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="18" height="18" rx="3" stroke="${color}" stroke-width="1.5" fill="${color}" fill-opacity="0.1"/><text x="7" y="16" fill="${color}" font-family="monospace" font-weight="900" font-size="10px">JS</text></svg>`;
  } else if (ext === "ts") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="18" height="18" rx="3" stroke="${color}" stroke-width="1.5" fill="${color}" fill-opacity="0.1"/><text x="7" y="16" fill="${color}" font-family="monospace" font-weight="900" font-size="10px">TS</text></svg>`;
  } else if (ext === "md") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none"><rect x="3" y="5" width="18" height="14" rx="2" stroke="${color}" stroke-width="1.5" fill="${color}" fill-opacity="0.1"/><path d="M7 15V9l3 3 3-3v6" stroke="${color}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/><path d="M16 11l2-2m0 0l2 2m-2-2v5" stroke="${color}" stroke-width="1.5" stroke-linecap="round"/></svg>`;
  } else if (ext === "css") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="18" height="18" rx="3" stroke="${color}" stroke-width="1.5" fill="${color}" fill-opacity="0.1"/><path d="M8 8h8M8 12h5" stroke="${color}" stroke-width="1.5" stroke-linecap="round"/></svg>`;
  } else if (ext === "html") {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="${color}" stroke-width="1.5"><path d="M8 9l-3 3 3 3M16 9l3 3-3 3M11.5 7l1 10" stroke-linecap="round"/></svg>`;
  } else {
    svg = `<svg width="15" height="15" viewBox="0 0 24 24" fill="none"><rect x="2" y="4" width="20" height="16" rx="2" stroke="${color}" stroke-width="1.5" fill="${color}" fill-opacity="0.05"/><path d="M9 12l3 2 3-2" stroke="${color}" stroke-width="1.5" stroke-linecap="round"/></svg>`;
  }

  const override = getIconForExt(ext, "");
  if (override) svg = override;

  return { svg, color };
}
