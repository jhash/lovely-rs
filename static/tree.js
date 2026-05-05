(function () {
  "use strict";
  var KEY_PREFIX = "lovely:open:";

  function readCookie(name) {
    var parts = document.cookie.split("; ");
    for (var i = 0; i < parts.length; i++) {
      if (parts[i].indexOf(name + "=") === 0) {
        return parts[i].substring(name.length + 1);
      }
    }
    return null;
  }

  document.addEventListener("toggle", function (e) {
    var el = e.target;
    if (!(el instanceof HTMLDetailsElement)) return;
    var uuid = el.dataset && el.dataset.uuid;
    if (!uuid) return;
    try {
      localStorage.setItem(KEY_PREFIX + uuid, el.open ? "open" : "closed");
    } catch (_err) { /* storage may be disabled */ }
  }, true);

  document.addEventListener("DOMContentLoaded", function () {
    var nodes = document.querySelectorAll("details[data-uuid]");
    for (var i = 0; i < nodes.length; i++) {
      var el = nodes[i];
      var v;
      try { v = localStorage.getItem(KEY_PREFIX + el.dataset.uuid); }
      catch (_err) { v = null; }
      if (v === "open") el.open = true;
      else if (v === "closed") el.open = false;
    }
    var token = readCookie("csrf_token");
    if (token && window.htmx) {
      window.htmx.config.headers = window.htmx.config.headers || {};
      window.htmx.config.headers["X-CSRF-Token"] = token;
    }
  });

  document.addEventListener("lovely:element-deleted", function (e) {
    var id = e.detail && (e.detail.uuid || e.detail.id);
    if (!id) return;
    var t = document.getElementById("tree-" + id);
    if (t) t.remove();
    var p = document.getElementById("preview-" + id);
    if (p) p.remove();
  });
})();
