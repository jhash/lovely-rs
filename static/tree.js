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

  // ---- Builder live preview ----
  // PATCH/MOVE responses include `HX-Trigger: preview-stale`. htmx
  // dispatches that as a plain event; we listen and reload the iframe
  // so the user sees their change without a full editor reload.
  document.addEventListener("preview-stale", function () {
    var ifr = document.getElementById("preview");
    if (ifr && ifr.contentWindow) {
      ifr.contentWindow.location.reload();
    }
  });

  // ---- Builder tree drag & drop (Sortable.js) ----
  function moveUrl(elementId) {
    // The edit page URL is /apps/{app}/pages/{page}/edit; the move URL
    // is the same prefix + /elements/{id}/move.
    var path = window.location.pathname;
    var m = path.match(/^(\/apps\/[^/]+\/pages\/[^/]+)\/edit/);
    if (!m) return null;
    return m[1] + "/elements/" + elementId + "/move";
  }

  function postMove(elementId, parentId, prevSiblingId) {
    var url = moveUrl(elementId);
    if (!url) return;
    var token = readCookie("csrf_token") || "";
    var body = new URLSearchParams();
    body.set("parent_id", parentId);
    body.set("prev_sibling", prevSiblingId || "");
    body.set("_csrf", token);
    fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "X-CSRF-Token": token,
      },
      body: body.toString(),
    }).then(function (r) {
      if (!r.ok) {
        console.warn("move failed", r.status);
        return;
      }
      // Re-trigger preview and refresh tree from server (single source).
      document.dispatchEvent(new CustomEvent("preview-stale"));
      if (window.htmx) {
        var t = document.getElementById("tree");
        if (t) window.htmx.trigger(t, "preview-stale");
      }
    });
  }

  function wireSortable(root) {
    if (!window.Sortable) return;
    var lists = root.querySelectorAll(".tree-children, .tree-root");
    lists.forEach(function (ul) {
      if (ul._sortable) return;
      ul._sortable = new window.Sortable(ul, {
        group: "lovely-tree",
        animation: 120,
        fallbackOnBody: true,
        invertSwap: true,
        ghostClass: "sortable-ghost",
        chosenClass: "sortable-chosen",
        onEnd: function (evt) {
          var li = evt.item;
          var elementId = li.getAttribute("data-element-id");
          var parentUl = li.parentElement;
          var parentId = parentUl && parentUl.getAttribute("data-parent-id");
          if (!elementId || !parentId) return;
          var prev = li.previousElementSibling;
          var prevId = prev ? prev.getAttribute("data-element-id") : null;
          postMove(elementId, parentId, prevId);
        },
      });
    });
  }

  document.addEventListener("DOMContentLoaded", function () {
    var tree = document.getElementById("tree");
    if (tree) wireSortable(tree);
  });

  // Cmd-Z / Cmd-Shift-Z on the editor page → POST /undo or /redo
  document.addEventListener("keydown", function (e) {
    if (!(e.metaKey || e.ctrlKey) || e.key.toLowerCase() !== "z") return;
    var path = window.location.pathname;
    var m = path.match(/^(\/apps\/[^/]+\/pages\/[^/]+)\/edit/);
    if (!m) return;
    e.preventDefault();
    var url = m[1] + (e.shiftKey ? "/redo" : "/undo");
    var token = readCookie("csrf_token") || "";
    var body = new URLSearchParams();
    body.set("_csrf", token);
    fetch(url, {
      method: "POST",
      headers: {
        "Content-Type": "application/x-www-form-urlencoded",
        "X-CSRF-Token": token,
      },
      body: body.toString(),
    }).then(function () {
      document.dispatchEvent(new CustomEvent("preview-stale"));
    });
  });
  document.addEventListener("htmx:afterSwap", function (e) {
    if (e.target && e.target.id === "tree") wireSortable(e.target);
  });
})();
