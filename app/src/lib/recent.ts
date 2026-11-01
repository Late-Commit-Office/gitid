// Recently checked folders, remembered in this webview only. Storage can be
// unavailable or cleared; the app works the same without it.

const KEY = "gitid.recentFolders";
const MAX = 6;

export function recentFolders(): string[] {
  try {
    const parsed: unknown = JSON.parse(localStorage.getItem(KEY) ?? "[]");
    return Array.isArray(parsed) ? parsed.filter((p): p is string => typeof p === "string") : [];
  } catch {
    return [];
  }
}

function save(list: string[]): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(list.slice(0, MAX)));
  } catch {
    /* storage unavailable: nothing to remember */
  }
}

export function rememberFolder(path: string): void {
  save([path, ...recentFolders().filter((p) => p !== path)]);
}

export function forgetFolder(path: string): void {
  save(recentFolders().filter((p) => p !== path));
}
