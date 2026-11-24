// Fills the table on releases.html from releases.md. Each "## vX.Y.Z (YYYY-MM-DD)" heading is one row,
// and the markdown under it is that release's notes.
(function () {
  var table = document.querySelector("[data-log]");
  if (!table) return;
  var releases = "https://github.com/Abdulkader-Safi/SiteVet-DesktopAPP/releases";

  function el(tag, className, text) {
    var node = document.createElement(tag);
    if (className) node.className = className;
    if (text) node.textContent = text;
    return node;
  }

  function cell(row, child) {
    var td = el("td");
    if (child) td.appendChild(child);
    row.appendChild(td);
    return td;
  }

  // The installer names cargo-packager gives each system.
  function downloads(tag) {
    var v = tag.replace(/^v/, "");
    var files = el("div", "files");
    [
      ["macOS .dmg", "SiteVet_" + v + "_aarch64.dmg"],
      ["Windows .exe", "sitevet-desktop_" + v + "_x64-setup.exe"],
      ["Linux .AppImage", "sitevet-desktop_" + v + "_x86_64.AppImage"],
    ].forEach(function (f) {
      var a = el("a", "", f[0]);
      a.href = releases + "/download/" + tag + "/" + f[1];
      files.appendChild(a);
    });
    return files;
  }

  fetch("releases.md", { cache: "no-cache" })
    .then(function (r) {
      if (!r.ok) throw new Error(r.status);
      return r.text();
    })
    .then(function (md) {
      var body = table.querySelector("tbody");
      md.split(/^## /m)
        .slice(1)
        .forEach(function (section, i) {
          var nl = section.indexOf("\n");
          var head = (nl < 0 ? section : section.slice(0, nl)).trim();
          var text = nl < 0 ? "" : section.slice(nl + 1).trim();
          var m = head.match(/^(\S+)\s*(?:\(([^)]*)\))?/);
          var tag = m ? m[1] : head;
          var row = el("tr");

          var link = el("a", i === 0 ? "tag latest" : "tag", tag);
          link.href = releases + "/tag/" + tag;
          cell(row, link);
          cell(row, el("span", "when", m && m[2] ? m[2] : ""));
          var notes = el("div", "md");
          // releases.md is this repository's own file, not visitor input.
          if (window.marked) notes.innerHTML = marked.parse(text);
          else notes.textContent = text;
          cell(row, notes);
          cell(row, downloads(tag));
          body.appendChild(row);
        });
    })
    .catch(function () {
      var row = el("tr");
      var td = cell(row, null);
      td.colSpan = 4;
      td.textContent = "The release list did not load. The same notes are on the GitHub releases page.";
      table.querySelector("tbody").appendChild(row);
    });
})();
