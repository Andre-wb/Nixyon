export const $ = (id) => document.getElementById(id);

export function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

// Цвет участника закреплён за его номером: после слияния и переименования цвета
// остальных не меняются.
export function speakerColor(id) {
  return `var(--speaker-${((id - 1) % 8 + 8) % 8 + 1})`;
}

export function dot(id) {
  const node = el("span", "dot");
  node.style.setProperty("--speaker-color", speakerColor(id));
  return node;
}
