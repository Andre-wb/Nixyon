import { errorText, invoke } from "./api.js";
import { $, dot, el, speakerColor } from "./dom.js";
import { clock } from "./format.js";
import { K, locale, t } from "./i18n.js";
import { onLanguageChange, openModal } from "./screens.js";
import { speakerName } from "./transcript.js";

let transcript = null;
let stats = null;

function number(value) {
  return new Intl.NumberFormat(locale(), { maximumFractionDigits: 0 }).format(value);
}

function percent(share) {
  return new Intl.NumberFormat(locale(), { style: "percent", maximumFractionDigits: 0 }).format(share);
}

function row(list, label, value) {
  const item = el("div", "stats-row");
  item.append(el("dt", null, label), el("dd", null, value));
  list.append(item);
}

function render() {
  if (!stats || !transcript) return;
  const summary = $("stats-summary");
  summary.replaceChildren();
  row(summary, t(K.stats_talk_time_total), clock(stats.talkTimeSec));
  row(summary, t(K.stats_interruptions_total), number(stats.interruptions));

  const list = $("stats-speakers");
  list.replaceChildren();
  const ordered = [...stats.speakers].sort((a, b) => b.talkTimeSec - a.talkTimeSec || a.id - b.id);
  for (const s of ordered) {
    const share = stats.talkTimeSec > 0 ? s.talkTimeSec / stats.talkTimeSec : 0;
    const card = el("li", "card stats-speaker");
    const head = el("div", "stats-head");
    head.append(
      dot(s.id),
      el("span", "speaker-name", speakerName(transcript, s.id)),
      el("span", "stats-share", percent(share)),
    );
    // Полоса дублирует долю, которая уже написана текстом, поэтому скрыта от чтеца.
    const track = el("div", "stats-track");
    track.setAttribute("aria-hidden", "true");
    const fill = el("div", "stats-fill");
    fill.style.setProperty("--speaker-color", speakerColor(s.id));
    fill.style.width = `${(share * 100).toFixed(1)}%`;
    track.append(fill);
    const details = el("dl", "stats-list");
    row(details, t(K.stats_talk_time), clock(s.talkTimeSec));
    row(details, t(K.stats_turns), number(s.turns));
    row(details, t(K.stats_pace), s.wordsPerMinute == null ? t(K.stats_no_value) : number(s.wordsPerMinute));
    row(details, t(K.stats_interruptions), number(s.interruptions));
    row(details, t(K.stats_monologues, { time: clock(stats.monologueSec) }), number(s.monologues));
    row(details, t(K.stats_longest_turn), clock(s.longestTurnSec));
    card.append(head, track, details);
    list.append(card);
  }
}

export async function openStats(current) {
  transcript = current;
  stats = null;
  $("stats-summary").replaceChildren();
  $("stats-speakers").replaceChildren();
  $("stats-error").textContent = "";
  openModal($("stats-sheet"));
  try {
    stats = await invoke("meeting_stats");
    render();
  } catch (error) {
    $("stats-error").textContent = errorText(error);
  }
}

export function initStats() {
  onLanguageChange(render);
}
