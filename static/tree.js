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

  // ---- Builder tree selection highlight ----
  // Click a tree row → mark its <li> aria-current and clear the previous
  // one immediately. htmx still swaps the inspector via the row's hx-get;
  // this is just instant local feedback so the magenta stripe never
  // lags behind a roundtrip.
  document.addEventListener("click", function (e) {
    var btn = e.target && e.target.closest && e.target.closest(".tree-row-button");
    if (!btn) return;
    var tree = document.getElementById("tree");
    if (!tree) return;
    var prev = tree.querySelectorAll('li[aria-current="true"]');
    for (var i = 0; i < prev.length; i++) prev[i].removeAttribute("aria-current");
    var li = btn.closest("li");
    if (li) li.setAttribute("aria-current", "true");
  });

  // ---- Builder live preview ----
  // The `#preview-canvas` div has its own `hx-trigger="preview-stale
  // from:body"` so it re-fetches when mutations land. No JS handler
  // needed here anymore — the iframe is gone.

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

  // ---- Tree keyboard nav (↑/↓/Enter/Delete) ----
  // Acts on the currently-selected (aria-current) tree row. Walks the
  // visible row order so collapsing detail elements stays out of scope
  // for now — the tree is fully expanded.
  function visibleRows() {
    var tree = document.getElementById("tree");
    if (!tree) return [];
    return Array.prototype.slice.call(
      tree.querySelectorAll("li[data-element-id]")
    );
  }
  function clickRow(li) {
    var btn = li && li.querySelector(".tree-row-button");
    if (btn) btn.click();
  }
  document.addEventListener("keydown", function (e) {
    var path = window.location.pathname;
    if (!/^\/apps\/[^/]+\/pages\/[^/]+\/edit/.test(path)) return;
    if (e.target && /INPUT|TEXTAREA|SELECT/.test(e.target.tagName)) return;
    var rows = visibleRows();
    if (!rows.length) return;
    var idx = -1;
    for (var i = 0; i < rows.length; i++) {
      if (rows[i].getAttribute("aria-current") === "true") {
        idx = i;
        break;
      }
    }
    if (e.key === "ArrowDown") {
      e.preventDefault();
      var next = rows[Math.min(rows.length - 1, idx + 1)];
      if (next) clickRow(next);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      var prev = rows[Math.max(0, idx - 1)];
      if (prev) clickRow(prev);
    } else if (e.key === "Delete" || e.key === "Backspace") {
      if (e.metaKey || e.ctrlKey || idx < 0) return;
      var li = rows[idx];
      if (!li) return;
      // Trigger the per-row Delete button if available.
      var actions = li.querySelector("details.tree-actions");
      if (!actions) return;
      var del = actions.querySelector(
        'form[hx-post*="/delete"] button[type="submit"]'
      );
      if (del) {
        e.preventDefault();
        del.click();
      }
    }
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
  // Server emits `lovely:select` after creating an element so the
  // editor can swap to that selection without a full reload. Detail:
  // {id: "<uuid>", focus: "text" | ""}. We update the URL, ask htmx to
  // re-fetch tree + inspector at the new sel, and focus the text
  // textarea afterwards if requested.
  document.body.addEventListener("lovely:select", function (e) {
    var id = e.detail && e.detail.id;
    if (!id) return;
    var focus = (e.detail && e.detail.focus) || "";
    var m = window.location.pathname.match(/^(\/apps\/[^/]+\/pages\/[^/]+)/);
    if (!m) return;
    var base = m[1];
    var inspectorUrl = base + "/inspector?sel=" + id;
    var treeUrl = base + "/tree?sel=" + id;

    if (window.history && window.history.replaceState) {
      window.history.replaceState(null, "", base + "/edit?sel=" + id);
    }

    // Update the outer aside elements' static hx-get URLs so any
    // subsequent `preview-stale` from-body events fetch with the new
    // selection rather than the initial-render one.
    var inspectorEl = document.getElementById("inspector");
    var treeEl = document.getElementById("tree");
    if (inspectorEl) inspectorEl.setAttribute("hx-get", inspectorUrl);
    if (treeEl) treeEl.setAttribute("hx-get", treeUrl);

    if (window.htmx) {
      // Re-process the asides so htmx picks up the new hx-get value.
      if (inspectorEl) window.htmx.process(inspectorEl);
      if (treeEl) window.htmx.process(treeEl);
      // Swap the bodies once now with the new selection.
      window.htmx.ajax("GET", inspectorUrl, {
        target: "#inspector",
        swap: "innerHTML",
      });
      window.htmx.ajax("GET", treeUrl, {
        target: "#tree",
        swap: "innerHTML",
      });
    }
    // The canvas div listens for `preview-stale` to re-fetch, so let
    // it know — selection events also imply the structure changed.
    document.dispatchEvent(new CustomEvent("preview-stale"));

    if (focus === "text") {
      // Wait for the inspector to swap in before focusing.
      var tries = 0;
      var poll = setInterval(function () {
        var ta = document.querySelector('#inspector textarea[name="text"]');
        if (ta) {
          ta.focus();
          var v = ta.value;
          ta.setSelectionRange(v.length, v.length);
          clearInterval(poll);
        } else if (++tries > 20) {
          clearInterval(poll);
        }
      }, 25);
    }
  });

  document.addEventListener("htmx:afterSwap", function (e) {
    if (e.target && e.target.id === "tree") wireSortable(e.target);
    // Slug live-validation: when the `.slug-feedback` element gets a
    // new fragment, mark the input aria-invalid + disable submit when
    // the response contains `.slug-error`.
    if (
      e.target &&
      e.target.classList &&
      e.target.classList.contains("slug-feedback")
    ) {
      var form = e.target.closest("form");
      if (!form) return;
      var input = form.querySelector("[data-slug-input]");
      var submit = form.querySelector('button[type="submit"]');
      var taken = e.target.querySelector(".slug-error") !== null;
      if (input) input.setAttribute("aria-invalid", taken ? "true" : "false");
      if (submit) submit.disabled = taken;
    }
  });
})();
