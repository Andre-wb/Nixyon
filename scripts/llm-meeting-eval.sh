#!/usr/bin/env bash
# Замер языковой модели на тестовых встречах: краткое содержание, решения и задачи.
# Использование: scripts/llm-meeting-eval.sh <llama-server> <папка результатов> <модель.gguf>...
# Модель работает только на процессоре (-ngl 0), как на телефоне без видеоускорителя.
# Для каждой встречи пишет ответ модели в <папка>/<модель>/<встреча>.txt и строку таблицы:
# время разбора текста и ответа, наибольшая память процесса, найдено задач из эталона.
# Задача считается найденной, если в одной строке ответа есть исполнитель и хотя бы одно
# слово из эталона; выдуманное и пропущенное по смыслу проверяется чтением ответов.
# Встречи meeting-en-*.txt идут с английской инструкцией, остальные с русской.
set -euo pipefail

SERVER="$1"
OUT="$2"
shift 2
DATA="$(cd "$(dirname "$0")" && pwd)/data/llm-meetings"
PORT=8099
SYSTEM='Ты составляешь протокол встречи по её расшифровке. Пиши по-русски и только то, что есть в расшифровке, ничего не додумывай. Ответ строго в таком виде:
Краткое содержание: 3–5 предложений.
Решения: список, каждое с новой строки через «- ». Если решений нет, напиши «Решений нет».
Задачи: список, каждая с новой строки: имя исполнителя, двоеточие, что сделать, в скобках срок. Например: «- Пётр: отправить отчёт поставщику (до вторника)». Исполнителя пиши так, как он назван в расшифровке. Если срок не назван, пиши «(срок не назван)». Если задач нет, напиши «Задач нет».'
SYSTEM_EN='You write the minutes of a meeting from its transcript. Write in English and only what is in the transcript, do not make anything up. Answer strictly in this format:
Summary: 3–5 sentences.
Decisions: a list, each on a new line starting with "- ". If there are no decisions, write "No decisions".
Tasks: a list, each on a new line: the assignee'"'"'s name, a colon, what to do, the deadline in parentheses. For example: "- Peter: send the report to the supplier (by Tuesday)". Write the assignee as they are named in the transcript. If no deadline is named, write "(no deadline named)". If there are no tasks, write "No tasks".'

mkdir -p "$OUT"
echo "| Модель | Встреча | Разбор текста, с | Ответ, с | Всего, с | Память, МБ | Задачи |"
echo "|---|---|---|---|---|---|---|"
for model in "$@"; do
  name="$(basename "$model" .gguf)"
  mkdir -p "$OUT/$name"
  "$SERVER" -m "$model" --port "$PORT" -c 8192 -ngl 0 --jinja --log-disable \
    >"$OUT/$name/server.log" 2>&1 &
  pid=$!
  trap 'kill "$pid" 2>/dev/null || true' EXIT
  until curl -sf "http://127.0.0.1:$PORT/health" >/dev/null; do
    kill -0 "$pid" 2>/dev/null || { echo "сервер не запустился: $model" >&2; exit 1; }
    sleep 1
  done
  for text in "$DATA"/meeting-*.txt; do
    meeting="$(basename "$text" .txt)"
    system="$SYSTEM"
    [[ "$meeting" == meeting-en-* ]] && system="$SYSTEM_EN"
    body="$(jq -n --arg system "$system" --rawfile user "$text" '{
      messages: [{role: "system", content: $system}, {role: "user", content: $user}],
      temperature: 0, max_tokens: 1024, cache_prompt: false,
      chat_template_kwargs: {enable_thinking: false}
    }')"
    reply="$(curl -sf "http://127.0.0.1:$PORT/v1/chat/completions" \
      -H 'Content-Type: application/json' -d "$body")"
    answer="$(jq -r '.choices[0].message.content' <<<"$reply")"
    printf '%s\n' "$answer" >"$OUT/$name/$meeting.txt"
    prompt_s="$(jq -r '.timings.prompt_ms / 1000 | . * 10 | round / 10' <<<"$reply")"
    answer_s="$(jq -r '.timings.predicted_ms / 1000 | . * 10 | round / 10' <<<"$reply")"
    total_s="$(jq -r '(.timings.prompt_ms + .timings.predicted_ms) / 1000 | . * 10 | round / 10' <<<"$reply")"
    rss_mb=$(( $(ps -o rss= -p "$pid") / 1024 ))
    lower="$(tr '[:upper:]' '[:lower:]' <<<"$answer")"
    found=0
    total=0
    while IFS= read -r task; do
      total=$((total + 1))
      who="$(jq -r '.who | ascii_downcase' <<<"$task" | tr '[:upper:]' '[:lower:]')"
      [ "$who" = "все" ] && who_re='все|каждый|всех|команда' || who_re="$who"
      lines="$(grep -iE -- "$who_re" <<<"$lower" || true)"
      [ -n "$lines" ] || continue
      while IFS= read -r word; do
        if grep -qi -- "$word" <<<"$lines"; then
          found=$((found + 1))
          break
        fi
      done < <(jq -r '.keywords[]' <<<"$task")
    done < <(jq -c '.tasks[]' "$DATA/$meeting.expected.json")
    echo "| $name | $meeting | $prompt_s | $answer_s | $total_s | $rss_mb | $found из $total |"
  done
  kill "$pid"
  wait "$pid" 2>/dev/null || true
  trap - EXIT
done
