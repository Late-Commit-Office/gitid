// Path display helpers. Paths stay as strings; these only shorten them for reading.

let home: string | null = null;

export function setHome(path: string | null): void {
  home = path ? normalize(path).replace(/\/+$/, "") : null;
}

function normalize(p: string): string {
  return p.replace(/\\/g, "/");
}

const caseInsensitive = /^(Win|Mac)/i.test(navigator.platform);

/** `/Users/ann/code/app` -> `~/code/app`. */
export function tilde(path: string): string {
  const p = normalize(path);
  if (!home) return path;
  const a = caseInsensitive ? p.toLowerCase() : p;
  const h = caseInsensitive ? home.toLowerCase() : home;
  if (a === h) return "~";
  if (a.startsWith(h + "/")) return "~" + p.slice(home.length);
  return path;
}

/** Last path segment, for compact labels. */
export function basename(path: string): string {
  const parts = normalize(path).replace(/\/+$/, "").split("/");
  return parts[parts.length - 1] || path;
}
