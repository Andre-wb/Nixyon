/* Startup guard: the interface is ES modules, so an old or disabled Android
   System WebView parses nothing at all and shows a blank dark page. This file
   is a classic script: it runs even when the modules never do, and explains
   what to do instead of leaving an empty screen. */
(function () {
  "use strict";

  var OVERLAY_ID = "nixyon-boot-error";
  var TIMEOUT_MS = 6000;

  function isReady() {
    return document.documentElement.getAttribute("data-nixyon-ready") === "1";
  }

  function hasOverlay() {
    return Boolean(document.getElementById(OVERLAY_ID));
  }

  function hideOverlay() {
    var node = document.getElementById(OVERLAY_ID);
    if (node && node.parentNode) node.parentNode.removeChild(node);
  }

  function showOverlay() {
    if (isReady() || hasOverlay()) return;
    var box = document.createElement("div");
    box.id = OVERLAY_ID;
    box.setAttribute("role", "alert");
    box.style.cssText = [
      "position:fixed",
      "inset:0",
      "z-index:2147483647",
      "display:flex",
      "flex-direction:column",
      "justify-content:center",
      "gap:16px",
      "padding:28px",
      "background:#101112",
      "color:#e8ebf0",
      "font:400 16px/1.55 system-ui,-apple-system,Segoe UI,Roboto,sans-serif",
      "-webkit-text-size-adjust:100%",
    ].join(";");

    var title = document.createElement("h1");
    title.textContent = "Nixyon не запустился";
    title.style.cssText = "margin:0;font-size:24px;line-height:1.25;font-weight:600";

    var hint = document.createElement("p");
    hint.textContent =
      "Интерфейсу нужен современный «Android System WebView» (Chrome 80 или новее). " +
      "Обновите «Android System WebView» и Chrome в Google Play, затем откройте приложение снова.";
    hint.style.cssText = "margin:0";

    var hintEn = document.createElement("p");
    hintEn.textContent =
      "This interface needs an up-to-date Android System WebView (Chrome 80 or newer). " +
      "Update Android System WebView and Chrome, then reopen the app.";
    hintEn.style.cssText = "margin:0;color:#9aa4b2;font-size:14px";

    var button = document.createElement("button");
    button.type = "button";
    button.textContent = "Обновить";
    button.style.cssText = [
      "align-self:flex-start",
      "padding:12px 22px",
      "border:0",
      "border-radius:14px",
      "background:#2f6fd0",
      "color:#fff",
      "font:inherit",
      "font-weight:600",
    ].join(";");
    button.addEventListener("click", function () {
      location.reload();
    });

    box.appendChild(title);
    box.appendChild(hint);
    box.appendChild(hintEn);
    box.appendChild(button);
    (document.body || document.documentElement).appendChild(box);
  }

  window.setTimeout(showOverlay, TIMEOUT_MS);
  // The modules may still finish after the guard fired (slow device): drop the message.
  window.setInterval(function () {
    if (isReady() && hasOverlay()) hideOverlay();
  }, 1000);
})();
