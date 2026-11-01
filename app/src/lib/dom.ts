import { t } from "./i18n";

// Tiny DOM helpers. Text always goes in through text nodes, never innerHTML,
// because config values (names, paths, URLs) are untrusted text.

type Child = Node | string | null | undefined | false;

interface Props {
  class?: string;
  title?: string;
  type?: string;
  attrs?: Record<string, string>;
  on?: Partial<{ [K in keyof HTMLElementEventMap]: (ev: HTMLElementEventMap[K]) => void }>;
}

export function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Props = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  if (props.class) node.className = props.class;
  if (props.title) node.title = props.title;
  if (props.type) node.setAttribute("type", props.type);
  for (const [k, v] of Object.entries(props.attrs ?? {})) node.setAttribute(k, v);
  for (const [name, handler] of Object.entries(props.on ?? {})) {
    node.addEventListener(name, handler as EventListener);
  }
  append(node, ...children);
  return node;
}

export function append(parent: Node, ...children: Child[]): void {
  for (const c of children) {
    if (c === null || c === undefined || c === false) continue;
    parent.appendChild(typeof c === "string" ? document.createTextNode(c) : c);
  }
}

export function clear(node: Node): void {
  while (node.firstChild) node.removeChild(node.firstChild);
}

/** A button that copies `text`, then briefly says so. */
export function copyButton(text: string): HTMLButtonElement {
  const button = el("button", { class: "copy", type: "button", title: t("copy.title") }, t("copy.button"));
  button.addEventListener("click", async () => {
    try {
      await navigator.clipboard.writeText(text);
      button.textContent = t("copy.done");
    } catch {
      button.textContent = t("copy.failed");
    }
    setTimeout(() => (button.textContent = t("copy.button")), 1500);
  });
  return button;
}
