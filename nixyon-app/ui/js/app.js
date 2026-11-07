import { invoke } from "./api.js";
import { initExport } from "./export.js";
import { failed, initHome } from "./home.js";
import { initLabels } from "./labels.js";
import { initMeetingTitle } from "./meeting-title.js";
import { initMeetings } from "./meetings.js";
import { initModels } from "./models.js";
import { unlockApp } from "./lock.js";
import { initPlayer } from "./player.js";
import { initProcessing } from "./processing.js";
import { applyHotwords, applySetting, initSettings } from "./settings.js";
import { handleModalBack, show } from "./screens.js";
import { initSearch } from "./search.js";
import { initStats } from "./stats.js";
import { initNames } from "./names.js";
import { initFillers } from "./fillers.js";
import { initSummary } from "./summary.js";
import { initTimeline } from "./timeline.js";
import { initTranscript } from "./transcript.js";
import { applyTheme } from "./theme.js";
import { initVoices } from "./voices.js";

async function start() {
  await unlockApp();
  const settings = await invoke("get_settings").catch(() => ({ language: "auto", theme: "auto" }));
  applyTheme(settings.theme);
  applySetting(settings.language);
  applyHotwords(settings.hotwords);
  initHome();
  initMeetings();
  initMeetingTitle();
  initLabels();
  initProcessing(failed);
  initTranscript();
  initSettings();
  initExport();
  initVoices();
  initStats();
  initSummary();
  initFillers();
  initNames();
  initSearch();
  initPlayer();
  initTimeline();
  history.replaceState(null, "", "#home");
  show("home");
  window.addEventListener("popstate", () => {
    if (handleModalBack()) return;
    // С экрана загрузки моделей, открытого из настроек, «Назад» возвращает в настройки.
    if (location.hash === "#settings") show("settings");
    else if (location.hash !== "#transcript" && location.hash !== "#voices" && location.hash !== "#models") show("home");
  });
  await initModels(Boolean(settings.models_prompt_seen));
}

start();
