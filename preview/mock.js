/* Local design-preview harness: imitates the Tauri commands with demo data.
   Everything the UI can do works here: recording with pause, import, transcription,
   meetings archive with search and labels, editing, emotion, filler words, re-checking
   a turn, voices, playback with speed and ranges, export, statistics, summary and
   the model download screen. Nothing is stored on disk and no microphone is opened —
   this is a showcase. */
(() => {
  const DEMO_KEY = "nixyon.onboarding.v1";
  const params = new URLSearchParams(location.search);
  const requestedScreen = params.get("screen") ?? "";

  // The onboarding screen is the first launch; every other screenshot route opens the
  // app as if the guide had been read. Without a route the real first-run flow stays:
  // the design-preview APK shows the guide once, then the home screen.
  if (requestedScreen === "onboarding") localStorage.removeItem(DEMO_KEY);
  else if (requestedScreen !== "") localStorage.setItem(DEMO_KEY, "1");

  const MEETING_SEC = 2748;
  const MAX_SAMPLE_SEC = 8;
  const OVERLAP = "overlap";

  const DIALOGUE = [
    {
      speaker: 1,
      text: "Давайте начнём с главного: нам нужно успеть показать новую версию до конца месяца, не потеряв качество распознавания.",
      emotion: "neutral",
    },
    {
      speaker: 2,
      text: "Я подготовил три варианта главного экрана. В каждом запись остаётся центральным действием, но контекст вокруг неё меняется.",
      emotion: "joy",
    },
    {
      speaker: 3,
      text: "Ну, с точки зрения инженерии самый безопасный путь — оставить текущий пайплайн и переработать только слой презентации.",
      emotion: "neutral",
    },
    {
      speaker: 1,
      text: "Тогда мы не трогаем распознавание и успеваем. Но нужно проверить, как новые состояния переживут длинные встречи и слабые устройства.",
      emotion: "neutral",
    },
    {
      speaker: 3,
      text: "Как бы, могу взять на себя нагрузочный прогон. Нужен список критических сценариев и хотя бы один час реальной записи.",
      emotion: "neutral",
    },
    {
      speaker: 2,
      text: "Предлагаю зафиксировать это как решение: интерфейс обновляем, движок не меняем. Ответственных и сроки запишем отдельно.",
      emotion: "joy",
    },
    {
      speaker: 1,
      text: "Согласна. Анна собирает финальный макет к среде, Лейла проверяет устойчивость, Михаил готовит релизную записку. Встречаемся в пятницу.",
      flags: [OVERLAP],
      emotion: "joy",
    },
  ];

  const SHORT_DIALOGUE = [
    { speaker: 1, text: "Так, запись пошла. Проверяю, что микрофон слышит комнату и уровень не уходит в красное." },
    { speaker: 2, text: "Слышно хорошо. Давай коротко проговорим итог: интерфейс обновляем, движок не трогаем." },
    { speaker: 1, text: "И проверяем длинные встречи на слабых устройствах. Всё остальное — как договорились." },
  ];

  const PLANNING_DIALOGUE = [
    { speaker: 1, text: "На неделе две встречи с заказчиком и один созвон по бюджету. Кто берёт дизайн-ревью?" },
    { speaker: 2, text: "Я возьму. Пришлю макеты к среде, а в четверг покажу команде." },
    { speaker: 1, text: "Хорошо, тогда в пятницу подводим итог и фиксируем план на следующую неделю." },
  ];

  const RETRO_DIALOGUE = [
    { speaker: 1, text: "Что шло хорошо: успели закрыть все задачи спринта и провели два демо." },
    { speaker: 3, text: "Что мешало: несколько правок приходили в последний день и ломали вёрстку." },
    { speaker: 2, text: "Предлагаю ввести заморозку за два дня до демо, чтобы не переделывать готовое." },
    { speaker: 1, text: "Согласна, попробуем в следующем спринте." },
  ];

  const SUMMARY =
    "Коротко о встрече\n\n" +
    "Участники договорились выпустить новую версию до конца месяца, не меняя движок распознавания: " +
    "перерабатывается только слой презентации.\n\n" +
    "Решения\n" +
    "• Интерфейс обновляем, пайплайн распознавания оставляем прежним.\n" +
    "• Перед выпуском проверяем поведение на длинных встречах и слабых устройствах.\n\n" +
    "Задачи\n" +
    "• Анна — финальный макет к среде.\n" +
    "• Лейла — нагрузочный прогон и список критических сценариев.\n" +
    "• Михаил — релизная записка.\n\n" +
    "Следующая встреча — в пятницу.";

  const FILLERS = ["как бы", "ну", "эм", "вот", "типа", "значит"];

  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const fail = (code, message = code) => Object.assign(new Error(message), { code });
  const clone = (value) => JSON.parse(JSON.stringify(value));
  const round = (value) => Math.round(value * 10) / 10;
  const isoNow = () => new Date().toISOString();

  // File name for exports and recordings: no colons, they break FAT-like storages.
  function stamp(date) {
    const pad = (n) => String(n).padStart(2, "0");
    return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}-${pad(date.getMinutes())}`;
  }

  function speakersOf(turns) {
    const byId = new Map();
    for (const turn of turns) {
      const item = byId.get(turn.speaker) ?? { id: turn.speaker, talk: 0, turns: 0, words: 0 };
      item.talk += Math.max(0, turn.end - turn.start);
      item.turns += 1;
      item.words += turn.text.trim().split(/\s+/).filter(Boolean).length;
      byId.set(turn.speaker, item);
    }
    const people = [
      { id: 1, name: "Анна", age: "adult", gender: "female" },
      { id: 2, name: "Михаил", age: "adult", gender: "male" },
      { id: 3, name: "Лейла", age: "adult", gender: "female" },
    ];
    return people
      .filter((person) => byId.has(person.id))
      .map((person) => {
        const item = byId.get(person.id);
        return {
          id: person.id,
          name: person.name,
          talk_time_sec: round(item.talk),
          turns: item.turns,
          words: item.words,
          age: person.age,
          gender: person.gender,
        };
      });
  }

  // Words with timing: the player and "skip silence" need them, and a paragraph gets
  // them when the transcription engine would hand them over.
  function wordsOf(text, start, end) {
    const parts = text.split(/\s+/).filter(Boolean);
    const step = (end - start) / Math.max(1, parts.length);
    return parts.map((word, index) => ({
      text: word,
      start: round(start + index * step),
      end: round(start + (index + 1) * step - step * 0.08),
    }));
  }

  // Turns spread evenly over the recording so the timeline and player stay honest.
  function turnsOf(durationSec, lines) {
    const step = durationSec / lines.length;
    return lines.map((line, index) => {
      const start = round(index * step + step * 0.06);
      const end = round((index + 1) * step - step * 0.06);
      return {
        id: index + 1,
        speaker: line.speaker,
        start,
        end,
        text: line.text,
        words: wordsOf(line.text, start, end),
        flags: line.flags ?? [],
        emotion: line.emotion ?? null,
      };
    });
  }

  function transcriptOf({ file, durationSec, createdAt, short = false, lines = null }) {
    const dialogue = lines ?? (short ? SHORT_DIALOGUE : DIALOGUE);
    const turns = turnsOf(durationSec, dialogue);
    return {
      schema_version: 1,
      created_at: createdAt,
      audio: { file, duration_sec: durationSec, sample_rate: 16000 },
      engine: { vad: "silero", asr: "gigaam-v3", embedding: "redimnet2" },
      speakers: speakersOf(turns),
      turns,
    };
  }

  function meetingOf({ id, title = null, durationSec, file, lines = null, short = false }) {
    return {
      title,
      transcript: transcriptOf({ file, durationSec, createdAt: id, short, lines }),
    };
  }

  // The archive the home screen lists; ids are the recording start, as in the engine.
  const archive = new Map(
    [
      meetingOf({
        id: "2026-11-06T18:40:00+04:00",
        durationSec: MEETING_SEC,
        file: "Стратегия продукта — 06.11.wav",
      }),
      meetingOf({
        id: "2026-11-05T10:15:00+04:00",
        title: "Планирование недели",
        durationSec: 1530,
        file: "Планирование недели.m4a",
        lines: PLANNING_DIALOGUE,
      }),
      meetingOf({
        id: "2026-11-03T16:05:00+04:00",
        durationSec: 892,
        file: "Созвон по бюджету.wav",
        short: true,
      }),
      meetingOf({
        id: "2026-10-29T09:00:00+04:00",
        title: "Ретро спринта",
        durationSec: 2210,
        file: "Ретро спринта.wav",
        lines: RETRO_DIALOGUE,
      }),
    ].map((meeting, index) => [
      meeting.transcript.created_at,
      { ...meeting, labels: index === 0 ? [1, 2] : index === 1 ? [1] : index === 3 ? [3] : [] },
    ]),
  );

  const LABELS = [
    { id: 1, name: "Продукт" },
    { id: 2, name: "Дизайн" },
    { id: 3, name: "Ретро" },
  ];

  const MODELS = [
    { key: "speech", size: 420_000_000, required: true, installed: true },
    { key: "live", size: 96_000_000, required: false, installed: true },
    { key: "denoiser", size: 18_000_000, required: false, installed: true },
    { key: "age", size: 54_000_000, required: false, installed: true },
    { key: "emotion", size: 132_000_000, required: false, installed: false },
    { key: "llm", size: 2_740_000_000, required: false, installed: true },
  ];

  const state = {
    language: params.get("lang") === "en" ? "en" : "ru",
    hotwords: ["Nixyon", "GigaAM"],
    audioCleanupDays: 30,
    modelsPromptSeen: true,
    speechLanguage: "auto",
    biometric: false,
    llmGpu: false,
    modelsDisabled: [],
    meetings: new Map(clone([...archive])),
    labels: clone(LABELS),
    voices: [
      {
        name: "Анна",
        hasSample: true,
        createdAt: "2026-10-12T12:20:00+04:00",
        meetings: [
          { id: "2026-11-06T18:40:00+04:00", title: null },
          { id: "2026-11-05T10:15:00+04:00", title: "Планирование недели" },
          { id: "2026-11-03T16:05:00+04:00", title: null },
          { id: "2026-10-29T09:00:00+04:00", title: "Ретро спринта" },
        ],
      },
      {
        name: "Михаил",
        hasSample: true,
        createdAt: "2026-10-19T09:05:00+04:00",
        meetings: [
          { id: "2026-11-06T18:40:00+04:00", title: null },
          { id: "2026-11-05T10:15:00+04:00", title: "Планирование недели" },
        ],
      },
      {
        name: "Лейла",
        hasSample: false,
        createdAt: "2026-10-28T16:45:00+04:00",
        meetings: [
          { id: "2026-11-06T18:40:00+04:00", title: null },
          { id: "2026-10-29T09:00:00+04:00", title: "Ретро спринта" },
        ],
      },
      { name: "Олег", hasSample: true, createdAt: "2026-11-02T11:10:00+04:00", meetings: [] },
    ],
    current: "2026-11-06T18:40:00+04:00",
    summary: null,
  };

  const current = () => state.meetings.get(state.current);

  function settings() {
    return {
      language: state.language,
      theme: "dark",
      hotwords: [...state.hotwords],
      audio_cleanup_days: state.audioCleanupDays,
      models_prompt_seen: state.modelsPromptSeen,
      speech_language: state.speechLanguage,
    };
  }

  function meetingsList() {
    return [...state.meetings.values()]
      .sort((a, b) => (a.transcript.created_at < b.transcript.created_at ? 1 : -1))
      .map((meeting) => ({
        id: meeting.transcript.created_at,
        title: meeting.title,
        duration_sec: meeting.transcript.audio.duration_sec,
        labels: [...meeting.labels],
        participants: meeting.transcript.speakers.map((speaker) => ({
          id: speaker.id,
          name: speaker.name ?? null,
        })),
      }));
  }

  function voicesView() {
    return { voices: clone(state.voices), maxSampleSec: MAX_SAMPLE_SEC, meetingsFailed: false };
  }

  function modelsStatus() {
    return {
      packs: clone(MODELS),
      metered: false,
      downloading: Boolean(downloading),
      pending: [],
      pendingAllowMetered: false,
      disabled: [...state.modelsDisabled],
    };
  }

  function statsOf(transcript) {
    const total = transcript.turns.reduce((sum, turn) => sum + Math.max(0, turn.end - turn.start), 0);
    const perSpeaker = new Map();
    let interruptions = 0;
    let monologueSec = 0;
    for (const turn of transcript.turns) {
      const item = perSpeaker.get(turn.speaker) ?? {
        id: turn.speaker,
        talkTimeSec: 0,
        turns: 0,
        words: 0,
        interruptions: 0,
        monologues: 0,
        longestTurnSec: 0,
      };
      const length = Math.max(0, turn.end - turn.start);
      const words = turn.text.trim().split(/\s+/).filter(Boolean).length;
      item.talkTimeSec += length;
      item.turns += 1;
      item.words += words;
      item.longestTurnSec = Math.max(item.longestTurnSec, length);
      if ((turn.flags ?? []).includes(OVERLAP)) {
        item.interruptions += 1;
        interruptions += 1;
      }
      if (length >= 60) {
        item.monologues += 1;
        monologueSec += length;
      }
      perSpeaker.set(turn.speaker, item);
    }
    const speakers = [...perSpeaker.values()].map((item) => ({
      id: item.id,
      talkTimeSec: round(item.talkTimeSec),
      turns: item.turns,
      wordsPerMinute: item.talkTimeSec > 0 ? Math.round(item.words / (item.talkTimeSec / 60)) : null,
      interruptions: item.interruptions,
      monologues: item.monologues,
      longestTurnSec: round(item.longestTurnSec),
    }));
    return {
      talkTimeSec: round(total),
      monologueSec: round(monologueSec),
      interruptions,
      speakers,
    };
  }

  // ---- recording -------------------------------------------------------
  let recording = null;
  let recorderTick = null;

  function liveLines(index) {
    const lines = [
      "Коллеги, начнём с главного вопроса —",
      "успеваем ли мы показать новую версию",
      "до конца месяца, не потеряв качество.",
      "Тогда движок не трогаем и обновляем интерфейс.",
    ];
    return lines.slice(0, Math.min(lines.length, index + 1));
  }

  function startRecording(args) {
    if (recording) throw fail("already_recording");
    recording = { startedAt: Date.now(), pausedMs: 0, pausedAt: 0, channel: args.onEvent ?? null };
    let tick = 0;
    recorderTick = setInterval(() => {
      if (recording?.pausedAt) return;
      tick += 1;
      const level = -42 + Math.sin(tick / 2.4) * 9 + Math.sin(tick / 7) * 4;
      recording?.channel?.onmessage?.({ type: "level", dbfs: round(level) });
      if (tick === 5) recording?.channel?.onmessage?.({ type: "live", lines: liveLines(0) });
      if (tick === 12) recording?.channel?.onmessage?.({ type: "live", lines: liveLines(1) });
      if (tick === 20) recording?.channel?.onmessage?.({ type: "live", lines: liveLines(2) });
    }, 250);
    return { started: true };
  }

  function pauseRecording() {
    if (!recording) throw fail("not_recording");
    if (recording.pausedAt) throw fail("not_recording");
    recording.pausedAt = Date.now();
    return true;
  }

  function resumeRecording() {
    if (!recording) throw fail("not_recording");
    if (!recording.pausedAt) throw fail("already_recording");
    recording.pausedMs += Date.now() - recording.pausedAt;
    recording.pausedAt = 0;
    return true;
  }

  function stopRecording() {
    if (!recording) throw fail("not_recording");
    clearInterval(recorderTick);
    recorderTick = null;
    const spent = Date.now() - recording.startedAt - recording.pausedMs;
    const startedAt = recording.startedAt;
    recording = null;
    const durationSec = Math.max(1, round(spent / 1000));
    const path = `/data/user/0/dev.demo.nixyon.design/files/recordings/rec-${startedAt}.wav`;
    files.set(path, { durationSec, name: `Запись ${stamp(new Date(startedAt))}`, short: true });
    return { path, durationSec };
  }

  // ---- import / transcription -----------------------------------------
  // Duration and display name of every recording and imported file this session.
  const files = new Map();

  // A single long job at a time, exactly like the engine: the UI relies on it.
  let job = null;

  async function importAudio(args) {
    if (job) throw fail("busy");
    const started = (job = { kind: "import", cancelled: false });
    try {
      const total = 96;
      for (let done = 0; done <= total; done += 12) {
        if (started.cancelled) throw fail("cancelled");
        args.onProgress?.onmessage?.({ phase: "importing", doneSec: done, totalSec: total });
        await sleep(150);
      }
      if (started.cancelled) throw fail("cancelled");
      const id = Date.now();
      const path = `/data/user/0/dev.demo.nixyon.design/files/recordings/import-${id}.m4a`;
      files.set(path, { durationSec: MEETING_SEC, name: "Планирование недели.m4a", short: false });
      return { path, name: "Планирование недели.m4a", durationSec: MEETING_SEC };
    } finally {
      job = null;
    }
  }

  // A new recording lands in the archive as the newest meeting and opens on screen.
  function addMeeting(transcript) {
    state.meetings.set(transcript.created_at, { title: null, transcript, labels: [] });
    state.current = transcript.created_at;
    return transcript;
  }

  async function transcribe(args) {
    if (job) throw fail("busy");
    const started = (job = { kind: "transcribe", cancelled: false });
    try {
      const known = files.get(args.path);
      const durationSec = known?.durationSec ?? MEETING_SEC;
      const phases = [
        ["prepare", 0],
        ["recognize", 0.25],
        ["recognize", 0.6],
        ["recognize", 1],
        ["voices", 1],
      ];
      for (const [phase, share] of phases) {
        if (started.cancelled) throw fail("cancelled");
        args.onProgress?.onmessage?.({ phase, doneSec: round(durationSec * share), totalSec: durationSec });
        await sleep(phase === "voices" ? 350 : 300);
      }
      if (started.cancelled) throw fail("cancelled");
      const transcript = transcriptOf({
        file: args.name || known?.name || "Встреча.wav",
        durationSec,
        createdAt: isoNow(),
        short: known?.short ?? false,
      });
      addMeeting(transcript);
      return { transcript, warnings: [] };
    } finally {
      job = null;
    }
  }

  // ---- playback --------------------------------------------------------
  const player = { open: false, channel: null, sec: 0, duration: 0, speed: 1, timer: null };

  function emitPlayer(event) {
    player.channel?.onmessage?.(event);
  }

  function stopPlayerTimer() {
    clearInterval(player.timer);
    player.timer = null;
  }

  function step() {
    player.sec = Math.min(player.duration, player.sec + 0.25 * player.speed);
    if (player.sec >= player.duration) {
      stopPlayerTimer();
      emitPlayer({ type: "position", sec: player.duration });
      emitPlayer({ type: "stopped", sec: player.duration });
      return;
    }
    emitPlayer({ type: "position", sec: round(player.sec) });
  }

  function openPlayback(args) {
    stopPlayerTimer();
    const meeting = state.meetings.get(args.meeting) ?? current();
    player.open = true;
    player.channel = args.onEvent ?? null;
    player.sec = 0;
    player.duration = meeting.transcript.audio.duration_sec;
    return { durationSec: player.duration };
  }

  function playAudio(args) {
    if (!player.open) throw fail("not_playing");
    stopPlayerTimer();
    if (typeof args?.fromSec === "number") player.sec = Math.min(Math.max(args.fromSec, 0), player.duration);
    if (player.sec >= player.duration) player.sec = 0;
    emitPlayer({ type: "playing", sec: player.sec });
    player.timer = setInterval(step, 250);
    return true;
  }

  function pauseAudio() {
    if (!player.open) throw fail("not_playing");
    stopPlayerTimer();
    emitPlayer({ type: "paused", sec: player.sec });
    return true;
  }

  function skipAudio(args) {
    if (!player.open) throw fail("not_playing");
    player.sec = Math.min(Math.max(0, player.sec + Number(args?.deltaSec ?? 0)), player.duration);
    emitPlayer({ type: "position", sec: round(player.sec) });
    return player.sec;
  }

  function setSpeed(args) {
    player.speed = Number(args?.speed) > 0 ? Number(args.speed) : 1;
    return player.speed;
  }

  function setRanges() {
    if (!player.open) throw fail("not_playing");
    return true;
  }

  function closePlayback() {
    stopPlayerTimer();
    player.open = false;
    player.channel = null;
    player.sec = 0;
  }

  // ---- transcript editing ---------------------------------------------
  function meetingArgs(args) {
    const meeting = args?.meeting ? state.meetings.get(args.meeting) : current();
    if (!meeting) throw fail("no_transcript");
    return meeting;
  }

  function saveTranscript(meeting, transcript) {
    meeting.transcript = transcript;
  }

  function speakerById(transcript, id) {
    return transcript.speakers.find((speaker) => speaker.id === id);
  }

  function renameSpeaker(args) {
    const meeting = meetingArgs(args);
    const next = clone(meeting.transcript);
    const speaker = speakerById(next, args.id);
    if (!speaker) throw fail("unknown_speaker");
    speaker.name = String(args.name ?? "").trim();
    saveTranscript(meeting, next);
    return next;
  }

  function mergeSpeakers(args) {
    const meeting = meetingArgs(args);
    const next = clone(meeting.transcript);
    const keep = speakerById(next, args.keep);
    const absorb = speakerById(next, args.absorb);
    if (!keep || !absorb) throw fail("unknown_speaker");
    if (keep.id === absorb.id) throw fail("same_speaker");
    let moved = 0;
    for (const turn of next.turns) {
      if (turn.speaker === absorb.id) {
        turn.speaker = keep.id;
        moved += 1;
      }
    }
    if (moved === 0) throw fail("unknown_speaker");
    keep.talk_time_sec = round(keep.talk_time_sec + absorb.talk_time_sec);
    keep.turns += absorb.turns;
    keep.words = (keep.words ?? 0) + (absorb.words ?? 0);
    next.speakers = next.speakers.filter((speaker) => speaker.id !== absorb.id);
    saveTranscript(meeting, next);
    return next;
  }

  function editTurnText(args) {
    const meeting = meetingArgs(args);
    const next = clone(meeting.transcript);
    const turn = next.turns.find((item) => item.id === args.id);
    if (!turn) throw fail("unknown_turn");
    const text = String(args.text ?? "").trim();
    if (!text) throw fail("text_empty");
    turn.text = text;
    turn.words = null;
    saveTranscript(meeting, next);
    return next;
  }

  function deleteTurn(args) {
    const meeting = meetingArgs(args);
    const next = clone(meeting.transcript);
    if (next.turns.length <= 1) throw fail("last_turn");
    const index = next.turns.findIndex((item) => item.id === args.id);
    if (index === -1) throw fail("unknown_turn");
    next.turns.splice(index, 1);
    saveTranscript(meeting, next);
    return next;
  }

  function setTurnEmotion(args) {
    const meeting = meetingArgs(args);
    const next = clone(meeting.transcript);
    const turn = next.turns.find((item) => item.id === args.id);
    if (!turn) throw fail("unknown_turn");
    turn.emotion = args.emotion ?? null;
    saveTranscript(meeting, next);
    return next;
  }

  // Re-checking one turn: listening again and then letting the model look at the text.
  async function reprocessTurn(args) {
    const meeting = meetingArgs(args);
    const turn = meeting.transcript.turns.find((item) => item.id === args.id);
    if (!turn) throw fail("unknown_turn");
    const total = 100;
    for (const done of [0, 30, 60, 100]) {
      args.onProgress?.onmessage?.({ stage: "listen", done, total });
      await sleep(260);
    }
    for (const done of [0, 50, 100]) {
      args.onProgress?.onmessage?.({ stage: "check", done, total });
      await sleep(240);
    }
    const recognized = turn.text;
    const correction = turn.text.includes("пайплайн") ? turn.text.replace("пайплайн", "конвейер") : null;
    return { recognized, correction };
  }

  // Filler words: the model proposes edits, the UI applies them on request.
  function removeFillers(args) {
    const meeting = meetingArgs(args);
    const ids = Array.isArray(args.ids) && args.ids.length ? args.ids : null;
    const changes = [];
    let skipped = 0;
    for (const turn of meeting.transcript.turns) {
      if (ids && !ids.includes(turn.id)) continue;
      let after = turn.text;
      for (const filler of FILLERS) {
        after = after.replace(new RegExp(`(^|\\s)${filler}(\\s|,|$)`, "gi"), " ").replace(/\s{2,}/g, " ");
      }
      after = after.trim();
      if (!after) {
        skipped += 1;
        continue;
      }
      if (after !== turn.text) changes.push({ id: turn.id, before: turn.text, after, delete: false });
    }
    return { changes, skipped, unanswered: 0 };
  }

  function applyTurnTexts(args) {
    const meeting = meetingArgs(args);
    const next = clone(meeting.transcript);
    let skipped = 0;
    for (const change of args.changes ?? []) {
      const turn = next.turns.find((item) => item.id === change.id);
      if (!turn || turn.text !== change.before) {
        skipped += 1;
        continue;
      }
      turn.text = change.after;
      turn.words = null;
    }
    saveTranscript(meeting, next);
    return { transcript: next, skipped };
  }

  // ---- meetings --------------------------------------------------------
  function openMeeting(args) {
    const meeting = state.meetings.get(args.meeting);
    if (!meeting) throw fail("no_transcript");
    state.current = args.meeting;
    state.summary = null;
    return { transcript: clone(meeting.transcript), title: meeting.title, has_audio: true };
  }

  function renameMeeting(args) {
    const meeting = state.meetings.get(args.meeting);
    if (!meeting) throw fail("no_transcript");
    const title = String(args.title ?? "").trim();
    meeting.title = title || null;
    return meeting.title;
  }

  function deleteMeeting(args) {
    if (!state.meetings.delete(args.meeting)) throw fail("no_transcript");
    closePlayback();
    const [first] = meetingsList();
    if (first) state.current = first.id;
    return true;
  }

  function searchMeetings(args) {
    const query = String(args.query ?? "").trim().toLowerCase();
    if (!query) return { hits: [], truncated: false };
    const wanted = new Set(args.labels ?? []);
    const hits = [];
    for (const meeting of meetingsList()) {
      if (wanted.size && ![...wanted].every((id) => meeting.labels.includes(id))) continue;
      const transcript = state.meetings.get(meeting.id).transcript;
      for (const turn of transcript.turns) {
        const at = turn.text.toLowerCase().indexOf(query);
        if (at < 0) continue;
        hits.push({
          meeting: meeting.id,
          title: meeting.title,
          turnId: turn.id,
          start: at,
          end: at + query.length,
          text: turn.text,
        });
        if (hits.length >= 24) return { hits, truncated: true };
      }
    }
    return { hits, truncated: false };
  }

  // ---- labels ----------------------------------------------------------
  function listLabels() {
    return clone(state.labels);
  }

  function createLabel(args) {
    const name = String(args.name ?? "").trim();
    if (!name) throw fail("label_empty");
    if (state.labels.some((label) => label.name.toLowerCase() === name.toLowerCase())) throw fail("label_exists");
    const label = { id: Math.max(0, ...state.labels.map((item) => item.id)) + 1, name };
    state.labels.push(label);
    state.labels.sort((a, b) => a.id - b.id);
    return clone(label);
  }

  function renameLabel(args) {
    const label = state.labels.find((item) => item.id === args.label);
    if (!label) throw fail("no_label");
    const name = String(args.name ?? "").trim();
    if (!name) throw fail("label_empty");
    label.name = name;
    return clone(label);
  }

  function deleteLabel(args) {
    state.labels = state.labels.filter((item) => item.id !== args.label);
    for (const meeting of state.meetings.values()) {
      meeting.labels = meeting.labels.filter((id) => id !== args.label);
    }
    return true;
  }

  function setMeetingLabels(args) {
    const meeting = state.meetings.get(args.meeting);
    if (!meeting) throw fail("no_transcript");
    const known = new Set(state.labels.map((label) => label.id));
    meeting.labels = (args.labels ?? []).filter((id) => known.has(id));
    return [...meeting.labels];
  }

  // ---- models ----------------------------------------------------------
  let downloading = null;

  function markInstalled(keys) {
    for (const pack of MODELS) if (keys.includes(pack.key)) pack.installed = true;
  }

  async function downloadModels(args) {
    if (downloading) throw fail("busy");
    const packs = (args.packs ?? []).filter((key) => MODELS.some((pack) => pack.key === key));
    const total = MODELS.filter((pack) => packs.includes(pack.key)).reduce((sum, pack) => sum + pack.size, 0);
    if (total === 0) throw fail("models_not_found");
    const started = (downloading = { cancelled: false });
    try {
      for (let share = 0; share <= 1.0001; share += 0.1) {
        if (started.cancelled) throw fail("cancelled");
        args.onProgress?.onmessage?.({ phase: "downloading", done: Math.round(total * share), total });
        await sleep(240);
      }
      markInstalled(packs);
      return true;
    } finally {
      downloading = null;
    }
  }

  function setModelsDisabled(args) {
    state.modelsDisabled = [...(args.packs ?? [])];
    return modelsStatus();
  }

  // ---- storage ---------------------------------------------------------
  function storageUsage() {
    const audio = [...state.meetings.values()].reduce((sum, meeting) => sum + meeting.transcript.audio.duration_sec * 32_000, 0);
    return {
      recordings: audio,
      transcripts: 1_180_000,
      models: 3_460_000_000,
      voices: 26_400,
      temporary: 41_200_000,
    };
  }

  // ---- voices ----------------------------------------------------------
  function rememberVoice(args) {
    const meeting = meetingArgs(args);
    const speaker = speakerById(meeting.transcript, args.speaker);
    if (!speaker?.name?.trim()) throw fail("speaker_unnamed");
    addVoice({ name: speaker.name, path: null });
    return true;
  }

  function addVoice(args) {
    const name = String(args.name ?? "").trim();
    if (!name) throw fail("text_empty");
    if (state.voices.some((voice) => voice.name.toLowerCase() === name.toLowerCase())) {
      throw fail("voice_name_taken");
    }
    state.voices.unshift({ name, hasSample: Boolean(args.path), createdAt: isoNow(), meetings: 0 });
    return voicesView();
  }

  function deleteVoice(args) {
    const name = String(args.name ?? "");
    const before = state.voices.length;
    state.voices = state.voices.filter((voice) => voice.name !== name);
    if (state.voices.length === before) throw fail("unknown_voice");
    return voicesView();
  }

  // ---- summary / names -------------------------------------------------
  let summaryJob = null;

  async function summarize(args) {
    if (summaryJob) throw fail("llm_busy");
    meetingArgs(args);
    const started = (summaryJob = { cancelled: false });
    try {
      const total = 100;
      for (const done of [0, 20, 40, 60]) {
        if (started.cancelled) throw fail("cancelled");
        args.onProgress?.onmessage?.({ phase: "reading", done, total });
        await sleep(220);
      }
      if (started.cancelled) throw fail("cancelled");
      args.onProgress?.onmessage?.({ phase: "writing", done: 80, total });
      await sleep(420);
      if (started.cancelled) throw fail("cancelled");
      args.onProgress?.onmessage?.({ phase: "writing", done: total, total });
      state.summary = SUMMARY;
      return SUMMARY;
    } finally {
      summaryJob = null;
    }
  }

  async function suggestNames(args) {
    const meeting = meetingArgs(args);
    const total = 100;
    for (const done of [0, 40, 70, 100]) {
      args.onProgress?.onmessage?.({ phase: "reading", done, total });
      await sleep(200);
    }
    const unnamed = meeting.transcript.speakers.filter((speaker) => !speaker.name?.trim());
    const names = ["Олег", "Марина", "Сергей", "Ирина"];
    return unnamed.map((speaker, index) => {
      const turn = meeting.transcript.turns.find((item) => item.speaker === speaker.id);
      return { speaker: speaker.id, name: names[index % names.length], turn: turn?.id ?? 1 };
    });
  }

  // ---- files -----------------------------------------------------------
  function exportTranscript(args) {
    const meeting = meetingArgs(args);
    const format = String(args?.format ?? "txt");
    const name = `Встреча — ${stamp(new Date(meeting.transcript.created_at))}.${format}`;
    return { name, path: `/data/user/0/dev.demo.nixyon.design/files/exports/${name}` };
  }

  // ---- command table ---------------------------------------------------
  const invoke = async (command, args = {}) => {
    switch (command) {
      case "get_settings":
        return settings();
      case "set_language":
        state.language = args.language ?? state.language;
        return settings();
      case "set_theme":
        return settings();
      case "set_speech_language":
        state.speechLanguage = args.choice ?? state.speechLanguage;
        return settings();
      case "unlock_storage":
        return null;
      case "biometric_status":
        return { enabled: state.biometric };
      case "set_biometric":
        state.biometric = Boolean(args.enabled);
        return null;
      case "open_biometric_settings":
        return true;
      case "llm_gpu_status":
        return { built: true, enabled: state.llmGpu, device: null };
      case "set_llm_gpu":
        state.llmGpu = Boolean(args.enabled);
        return null;
      case "set_hotwords": {
        state.hotwords = String(args.text ?? "")
          .split(/\r?\n/)
          .map((word) => word.trim())
          .filter(Boolean)
          .slice(0, 50);
        return settings();
      }
      case "set_audio_cleanup":
        state.audioCleanupDays = args.days ?? null;
        return settings();
      case "clean_old_audio":
        return 0;
      case "set_system_bars":
        return null;

      // models
      case "models_status":
        return modelsStatus();
      case "download_models":
        return downloadModels(args);
      case "cancel_model_download":
        if (downloading) downloading.cancelled = true;
        return true;
      case "set_models_disabled":
        return setModelsDisabled(args);
      case "set_models_prompt_seen":
        state.modelsPromptSeen = true;
        return settings();

      // storage
      case "storage_usage":
        return storageUsage();
      case "remove_all_audio":
        return true;
      case "clear_temporary_files":
        return true;

      // meetings
      case "list_meetings":
        return meetingsList();
      case "open_meeting":
        return openMeeting(args);
      case "rename_meeting":
        return renameMeeting(args);
      case "delete_meeting":
        return deleteMeeting(args);
      case "search_meetings":
        return searchMeetings(args);

      // labels
      case "list_labels":
        return listLabels();
      case "create_label":
        return createLabel(args);
      case "rename_label":
        return renameLabel(args);
      case "delete_label":
        return deleteLabel(args);
      case "set_meeting_labels":
        return setMeetingLabels(args);

      // recording
      case "recovered_recording":
        return null;
      case "start_recording":
        return startRecording(args);
      case "pause_recording":
        return pauseRecording();
      case "resume_recording":
        return resumeRecording();
      case "stop_recording":
        return stopRecording();
      case "import_audio":
        return importAudio(args);
      case "cancel_import":
        if (job?.kind === "import") job.cancelled = true;
        return true;
      case "transcribe":
        return transcribe(args);
      case "cancel_transcription":
        if (job?.kind === "transcribe") job.cancelled = true;
        return true;

      // transcript
      case "rename_speaker":
        return renameSpeaker(args);
      case "merge_speakers":
        return mergeSpeakers(args);
      case "edit_turn_text":
        return editTurnText(args);
      case "delete_turn":
        return deleteTurn(args);
      case "set_turn_emotion":
        return setTurnEmotion(args);
      case "reprocess_turn":
        return reprocessTurn(args);
      case "remove_filler_words":
        return removeFillers(args);
      case "apply_turn_texts":
        return applyTurnTexts(args);
      case "meeting_stats":
        return statsOf(meetingArgs(args).transcript);
      case "search_transcript": {
        const query = String(args.query ?? "").toLowerCase();
        if (!query) return [];
        const hits = [];
        for (const turn of meetingArgs(args).transcript.turns) {
          const at = turn.text.toLowerCase().indexOf(query);
          if (at >= 0) hits.push({ turnId: turn.id, start: at, end: at + query.length });
        }
        return hits;
      }

      // playback
      case "open_playback":
        return openPlayback(args);
      case "play_audio":
        return playAudio(args);
      case "pause_audio":
        return pauseAudio();
      case "skip_audio":
        return skipAudio(args);
      case "set_playback_speed":
        return setSpeed(args);
      case "set_playback_ranges":
        return setRanges(args);
      case "close_playback":
        return closePlayback();

      // voices
      case "list_voices":
        return voicesView();
      case "add_voice":
        return addVoice(args);
      case "delete_voice":
        return deleteVoice(args);
      case "remember_voice":
        return rememberVoice(args);
      case "discard_voice_recording":
        return true;

      // summary / names
      case "summary_available":
        return true;
      case "meeting_summary":
        return state.summary;
      case "summarize_meeting":
        return summarize(args);
      case "cancel_summary":
        if (summaryJob) summaryJob.cancelled = true;
        return true;
      case "suggest_speaker_names":
        return suggestNames(args);

      // files
      case "export_transcript":
        return exportTranscript(args);
      case "save_export":
        return true;
      case "share_export":
        return true;
      case "open_app_settings":
        return true;
      default:
        throw fail("unknown_command", `unknown command: ${command}`);
    }
  };

  class Channel {
    constructor() {
      this.onmessage = null;
    }
  }

  window.__TAURI__ = { core: { invoke, Channel } };

  // ---- screenshot routes ----------------------------------------------
  const delay = sleep;
  const waitFor = async (selector, timeout = 4000) => {
    const started = performance.now();
    while (performance.now() - started < timeout) {
      const node = document.querySelector(selector);
      if (node && !node.hidden) return node;
      await delay(40);
    }
    return null;
  };
  const click = (selector) => document.querySelector(selector)?.click();

  window.addEventListener("load", async () => {
    const screen = params.get("screen") ?? "home";
    if (screen === "home" || screen === "onboarding") return;
    await delay(500);
    if (screen === "processing") {
      click("#import");
      return;
    }
    if (screen === "models") {
      click("#models-open");
      return;
    }
    if (screen === "search") {
      const field = await waitFor("#meetings-search");
      field.value = "интерфейс";
      field.dispatchEvent(new Event("input", { bubbles: true }));
      return;
    }
    const last = await waitFor("#meetings-list .meeting-row");
    last?.click();
    await waitFor("#turns .turn");
    await delay(600);
    if (screen === "transcript") return;
    if (screen === "voices") {
      history.pushState(null, "", "#voices");
      click("#open-voices");
    } else if (screen === "settings") {
      click("#open-settings");
    } else if (screen === "export") {
      click("#open-export");
    } else if (screen === "stats") {
      click("#open-stats");
    } else if (screen === "speaker") {
      click("#speakers .speaker-row");
    } else if (screen === "summary") {
      click("#open-summary");
    } else if (screen === "labels") {
      click("#open-labels");
    } else if (screen === "fillers") {
      click("#open-fillers");
    } else if (screen === "emotion") {
      click("#turns .turn-emotion");
    } else if (screen === "turn") {
      click("#turns .turn-edit");
    } else if (screen === "check") {
      click("#turns .turn-check");
      await delay(400);
      click("#turn-check-start");
    } else if (screen === "player") {
      click("#player-toggle");
    }
  });
})();
