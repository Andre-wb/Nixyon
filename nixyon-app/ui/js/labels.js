import { errorText, invoke } from "./api.js";
import { $, el } from "./dom.js";
import { K, t } from "./i18n.js";
import { onLanguageChange, openModal } from "./screens.js";

const SVG = "http://www.w3.org/2000/svg";
const PENCIL = "M4 20h4L19 9l-4-4L4 16v4zM13 7l4 4";
const TRASH = "M4 7h16M10 11v6M14 11v6M6 7l1 13h10l1-13M9 7V4h6v3";

let meeting = null;
let labels = [];
let assigned = new Set();
// id метки, у которой вместо названия открыто поле переименования.
let editing = null;
let pendingDelete = null;

function icon(d) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS(SVG, "path");
  path.setAttribute("d", d);
  svg.append(path);
  return svg;
}

function iconButton(d, label, className) {
  const button = el("button", className ? `icon-button ${className}` : "icon-button");
  button.type = "button";
  button.setAttribute("aria-label", label);
  button.title = label;
  button.append(icon(d));
  return button;
}

function showError(error) {
  $("labels-error").textContent = error ? errorText(error) : "";
}

function checkRow(label) {
  const check = el("label", "label-check");
  const box = el("input", "select-circle");
  box.type = "checkbox";
  box.checked = assigned.has(label.id);
  box.addEventListener("change", () => toggle(label.id, box));
  check.append(box, el("span", "label-name", label.name));
  const rename = iconButton(PENCIL, t(K.labels_rename, { name: label.name }));
  rename.addEventListener("click", () => {
    editing = label.id;
    showError(null);
    render();
    $("label-edit")?.focus();
  });
  const remove = iconButton(TRASH, t(K.labels_delete, { name: label.name }), "danger-icon");
  remove.addEventListener("click", () => askDelete(label));
  return [check, rename, remove];
}

function editRow(label) {
  const input = el("input", "search-input");
  input.id = "label-edit";
  input.type = "text";
  input.maxLength = 40;
  input.autocomplete = "off";
  input.enterKeyHint = "done";
  input.value = label.name;
  input.setAttribute("aria-label", t(K.labels_name));
  input.addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      rename(label.id, input.value);
    } else if (e.key === "Escape") {
      // Esc отменяет переименование, а не закрывает весь лист.
      e.preventDefault();
      editing = null;
      render();
    }
  });
  const save = el("button", "button button-plain", t(K.labels_save));
  save.type = "button";
  save.addEventListener("click", () => rename(label.id, input.value));
  return [input, save];
}

function render() {
  const list = $("labels-list");
  list.replaceChildren();
  for (const label of labels) {
    const item = el("li", label.id === editing ? "label-row label-edit" : "label-row");
    item.append(...(label.id === editing ? editRow(label) : checkRow(label)));
    list.append(item);
  }
  list.hidden = labels.length === 0;
  $("labels-empty").hidden = labels.length > 0;
}

async function reload() {
  labels = await invoke("list_labels");
  const known = new Set(labels.map((l) => l.id));
  for (const id of [...assigned]) if (!known.has(id)) assigned.delete(id);
  if (editing !== null && !known.has(editing)) editing = null;
  render();
}

// Лист меток встречи: какие назначены, берётся из индекса через список встреч.
export async function openLabels(id) {
  meeting = id;
  editing = null;
  labels = [];
  assigned = new Set();
  $("label-new").value = "";
  showError(null);
  render();
  openModal($("labels-sheet"));
  try {
    const [all, meetings] = await Promise.all([invoke("list_labels"), invoke("list_meetings")]);
    if (meeting !== id) return;
    labels = all;
    assigned = new Set(meetings.find((m) => m.id === id)?.labels ?? []);
    render();
  } catch (error) {
    showError(error);
  }
}

async function toggle(id, box) {
  const next = new Set(assigned);
  if (box.checked) next.add(id);
  else next.delete(id);
  try {
    assigned = new Set(await invoke("set_meeting_labels", { meeting, labels: [...next] }));
    showError(null);
  } catch (error) {
    box.checked = assigned.has(id);
    showError(error);
    // Метку могли удалить: список обновляется, чтобы не предлагать её снова.
    await reload().catch(() => {});
  }
}

async function add() {
  try {
    await invoke("create_label", { name: $("label-new").value });
    $("label-new").value = "";
    showError(null);
    await reload();
  } catch (error) {
    showError(error);
  }
}

async function rename(id, name) {
  try {
    await invoke("rename_label", { label: id, name });
    editing = null;
    showError(null);
    await reload();
  } catch (error) {
    showError(error);
  }
}

function askDelete(label) {
  pendingDelete = label.id;
  $("label-delete-text").textContent = t(K.labels_delete_confirm, { name: label.name });
  openModal($("label-delete-confirm"));
}

async function confirmDelete() {
  $("label-delete-confirm").close();
  if (pendingDelete === null) return;
  const id = pendingDelete;
  pendingDelete = null;
  try {
    await invoke("delete_label", { label: id });
    assigned.delete(id);
    showError(null);
    await reload();
  } catch (error) {
    showError(error);
  }
}

export function initLabels() {
  $("label-add").addEventListener("click", add);
  $("label-new").addEventListener("keydown", (e) => {
    if (e.key === "Enter") {
      e.preventDefault();
      add();
    }
  });
  $("label-delete-yes").addEventListener("click", confirmDelete);
  $("label-delete-no").addEventListener("click", () => {
    pendingDelete = null;
    $("label-delete-confirm").close();
  });
  onLanguageChange(render);
}
