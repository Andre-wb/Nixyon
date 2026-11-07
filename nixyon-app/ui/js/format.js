import { locale } from "./i18n.js";

// Время в записи: 0:05, 12:40, от часа 1:02:03; доли секунды отбрасываются, как в плеерах.
export function clock(seconds) {
  const total = Number.isFinite(seconds) && seconds > 0 ? Math.floor(seconds) : 0;
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = String(total % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${s}` : `${m}:${s}`;
}

// Дата расшифровки в поясе устройства и на языке интерфейса.
export function dateTime(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat(locale(), { dateStyle: "medium", timeStyle: "short" }).format(date);
}

// Дата и время встречи полностью, «7 ноября 2026 г., 10:00»: название встречи по умолчанию.
export function longDateTime(iso) {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return new Intl.DateTimeFormat(locale(), { dateStyle: "long", timeStyle: "short" }).format(date);
}

// Размер файла на языке интерфейса: от гигабайта с одним знаком, меньше в мегабайтах.
// Единицы десятичные, как в настройках памяти Android.
export function bytes(count) {
  const n = Number.isFinite(count) && count > 0 ? count : 0;
  const giga = n >= 1e9;
  const value = giga ? n / 1e9 : n / 1e6;
  return new Intl.NumberFormat(locale(), {
    style: "unit",
    unit: giga ? "gigabyte" : "megabyte",
    unitDisplay: "short",
    maximumFractionDigits: giga || value < 10 ? 1 : 0,
  }).format(value);
}
