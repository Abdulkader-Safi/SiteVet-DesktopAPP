// Landing page behaviour: theme switch, themed screenshots, screen tabs, the ticker's pause
// button, and download links pointed at the real files in the latest release.
(function () {
  var root = document.documentElement;
  var repo = "Abdulkader-Safi/SiteVet-DesktopAPP";

  // Theme switch. The head script already applied the saved or system theme.
  var toggle = document.querySelector("[data-theme-toggle]");
  function labelToggle() {
    if (!toggle) return;
    var next = root.dataset.theme === "dark" ? "light" : "dark";
    toggle.setAttribute("aria-label", "Switch to " + next + " theme");
  }
  // Screenshots with a light version follow the theme.
  function themeImages() {
    var key = root.dataset.theme === "light" ? "light" : "dark";
    var heroLight = document.getElementById("hero-light");
    if (heroLight) heroLight.media = key === "light" ? "all" : "not all";
    document.querySelectorAll("img[data-light]").forEach(function (img) {
      var src = img.dataset[key];
      if (src && img.getAttribute("src") !== src) img.src = src;
    });
  }
  labelToggle();
  themeImages();
  if (toggle) {
    toggle.addEventListener("click", function () {
      root.dataset.theme = root.dataset.theme === "dark" ? "light" : "dark";
      try {
        localStorage.setItem("theme", root.dataset.theme);
      } catch (e) {}
      labelToggle();
      themeImages();
      if (window.setThemeColor) window.setThemeColor();
    });
  }

  // The ticker moves until paused (WCAG 2.2.2); the button works from the keyboard.
  var tickerButton = document.querySelector("[data-ticker-toggle]");
  if (tickerButton) {
    tickerButton.addEventListener("click", function () {
      var paused = tickerButton.getAttribute("aria-pressed") !== "true";
      tickerButton.setAttribute("aria-pressed", paused ? "true" : "false");
      tickerButton.textContent = paused ? "Play" : "Pause";
      tickerButton.closest(".ticker").classList.toggle("paused", paused);
    });
  }

  // Screen tabs, with arrow keys moving between them.
  var tabs = Array.prototype.slice.call(
    document.querySelectorAll('[role="tab"]'),
  );
  function select(tab, focus) {
    tabs.forEach(function (t) {
      var on = t === tab;
      t.setAttribute("aria-selected", on ? "true" : "false");
      t.tabIndex = on ? 0 : -1;
      document.getElementById(t.getAttribute("aria-controls")).hidden = !on;
    });
    if (focus) tab.focus();
  }
  tabs.forEach(function (tab, i) {
    tab.addEventListener("click", function () {
      select(tab, false);
    });
    tab.addEventListener("keydown", function (e) {
      var j = null;
      if (e.key === "ArrowRight") j = (i + 1) % tabs.length;
      if (e.key === "ArrowLeft") j = (i - 1 + tabs.length) % tabs.length;
      if (e.key === "Home") j = 0;
      if (e.key === "End") j = tabs.length - 1;
      if (j === null) return;
      e.preventDefault();
      select(tabs[j], true);
    });
  });

  // Mark the visitor's system and name it on the main button.
  var ua = navigator.userAgent;
  var os = /Windows/i.test(ua)
    ? "win"
    : /Mac/i.test(ua)
      ? "mac"
      : /Linux|X11/i.test(ua)
        ? "linux"
        : null;
  var names = { mac: "macOS", win: "Windows", linux: "Linux" };
  if (os) {
    var mine = document.querySelector('.dl a[data-os="' + os + '"]');
    if (mine) mine.classList.add("mine");
    var label = document.querySelector("[data-primary-label]");
    if (label) label.textContent = "Download for " + names[os];
  }

  // Point each card at its file in the latest release, with the file name and size.
  if (!document.querySelector(".dl")) return;
  fetch("https://api.github.com/repos/" + repo + "/releases/latest")
    .then(function (r) {
      return r.ok ? r.json() : null;
    })
    .then(function (rel) {
      if (!rel) return;
      document.querySelectorAll("[data-version]").forEach(function (el) {
        el.textContent = rel.tag_name;
      });
      var pick = { mac: /\.dmg$/, win: /setup\.exe$/, linux: /\.AppImage$/ };
      Object.keys(pick).forEach(function (key) {
        var asset = (rel.assets || []).find(function (a) {
          return pick[key].test(a.name);
        });
        var link = document.querySelector('.dl a[data-os="' + key + '"]');
        if (!asset || !link) return;
        link.href = asset.browser_download_url;
        var mb = (asset.size / 1048576).toFixed(1);
        link.querySelector("[data-meta]").textContent =
          asset.name + " · " + mb + " MB";
      });
    })
    .catch(function () {
      // Offline or rate-limited: the cards still open the latest release page.
    });
})();
