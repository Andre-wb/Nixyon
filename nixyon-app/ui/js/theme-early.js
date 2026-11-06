// Тема до первой отрисовки, иначе экран мигнёт светлым. Источник выбора: settings.json
// через ядро, здесь его копия из localStorage; после загрузки настроек её сверяет theme.js.
try {
  const theme = localStorage.getItem("nixyon.theme");
  if (theme === "light" || theme === "dark") document.documentElement.dataset.theme = theme;
} catch {
  // Хранилище недоступно: тема по системе до загрузки настроек.
}
