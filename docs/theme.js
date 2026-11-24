// Runs in <head> before first paint: the saved theme, else the system's, plus the browser bar colour.
(function () {
  var root = document.documentElement;
  var theme = null;
  try {
    theme = localStorage.getItem("theme");
  } catch (e) {}
  if (theme !== "light" && theme !== "dark") {
    theme = matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
  }
  root.dataset.theme = theme;
  root.classList.add("js");

  // Shared with site.js and releases.html so the bar follows every switch.
  window.setThemeColor = function () {
    var meta = document.querySelector('meta[name="theme-color"]');
    if (meta) meta.content = root.dataset.theme === "light" ? "#EFEEE8" : "#0B0B0C";
  };
  window.setThemeColor();
})();
