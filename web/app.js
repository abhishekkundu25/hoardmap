"use strict";

// Pure helpers.

const UNITS = ["KB", "MB", "GB", "TB", "PB"];

// Base 1000 like Finder: "12 bytes", "340 KB", "34.5 MB", "1.2 GB".
function formatBytes(n) {
  if (!Number.isFinite(n)) return "";
  if (n < 1000) return n === 1 ? "1 byte" : `${n} bytes`;
  let value = n / 1000;
  let unit = 0;
  while (value >= 1000 && unit < UNITS.length - 1) {
    value /= 1000;
    unit += 1;
  }
  const text = value < 100 ? value.toFixed(1) : Math.round(value).toString();
  return `${text} ${UNITS[unit]}`;
}

function pad2(n) {
  return n < 10 ? `0${n}` : `${n}`;
}

// Local YYYY-MM-DD HH:MM from seconds since the epoch.
function formatDate(seconds) {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  const d = new Date(seconds * 1000);
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())} ${pad2(d.getHours())}:${pad2(d.getMinutes())}`;
}

// Share of `whole` as a number between 0 and 100.
function percent(part, whole) {
  if (!whole || !Number.isFinite(part / whole)) return 0;
  return Math.max(0, Math.min(100, (part / whole) * 100));
}

function formatPercent(p) {
  return p >= 99.95 ? "100%" : `${p.toFixed(1)}%`;
}

function formatElapsed(ms) {
  const s = ms / 1000;
  return s < 10 ? `${s.toFixed(1)} s` : `${Math.round(s)} s`;
}

function formatCount(n) {
  return Number(n).toLocaleString();
}

function joinPath(parent, name) {
  return parent.endsWith("/") ? parent + name : `${parent}/${name}`;
}

// Rows of the list in display order: every node reachable from `rootId` through
// expanded nodes whose children are loaded.
function flatten(cache, expanded, rootId) {
  const rows = [];
  const walk = (id, depth, parent, path) => {
    const node = cache.get(id);
    if (!node) return;
    rows.push({ id, depth, node, parent, path });
    if (expanded.has(id) && Array.isArray(node.children)) {
      for (const child of node.children) {
        walk(child.id, depth + 1, node, joinPath(path, child.name));
      }
    }
  };
  const root = cache.get(rootId);
  if (root) walk(rootId, 0, null, root.path || root.name);
  return rows;
}

// Mixes a #rrggbb colour toward white (amount > 0) or black (amount < 0).
function shade(hex, amount) {
  const n = parseInt(hex.slice(1), 16);
  if (Number.isNaN(n) || hex.length !== 7) return hex;
  const target = amount > 0 ? 255 : 0;
  const t = Math.abs(amount);
  const mix = (c) => Math.round(c + (target - c) * t);
  const r = mix((n >> 16) & 255);
  const g = mix((n >> 8) & 255);
  const b = mix(n & 255);
  return `rgb(${r},${g},${b})`;
}

function bytesOf(node, mode) {
  return mode === "physical" ? node.physical : node.size;
}

// Store and DOM.

const store = {
  status: null,
  selection: null,
  root: 0,
  mode: "logical",
  highlightExt: null,
  layout: null,
  message: null,
  loadedPath: null,
};

const nodes = new Map();
const expanded = new Set();
const pending = new Map();
// Bumped when the tree is replaced so answers for the old tree are dropped.
let generation = 0;

const $ = (id) => document.getElementById(id);
const el = {
  path: $("path"),
  scanForm: $("scan-form"),
  scan: $("scan"),
  rescan: $("rescan"),
  open: $("open"),
  mode: $("mode"),
  zoom: $("zoom"),
  root: $("root"),
  status: $("status"),
  list: $("list"),
  rows: $("rows"),
  caption: $("treemap-caption"),
  pane: $("treemap-pane"),
  canvas: $("canvas"),
  tooltip: $("tooltip"),
  extRows: $("ext-rows"),
  largestRows: $("largest-rows"),
  hint: $("hint"),
  errors: $("errors"),
  errorRows: $("error-rows"),
  errorsTitle: $("errors-title"),
  errorsClose: $("errors-close"),
};

async function getJSON(url) {
  const response = await fetch(url, { cache: "no-store" });
  const body = await response.json().catch(() => ({}));
  if (!response.ok) throw new Error(body.error || `${response.status} ${response.statusText}`);
  return body;
}

async function postJSON(url, data) {
  const response = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(data),
  });
  if (!response.ok) {
    const body = await response.json().catch(() => ({}));
    throw new Error(body.error || `${response.status} ${response.statusText}`);
  }
  return response.status === 204 ? null : response.json();
}

// A node with its path and children, fetched once. Child summaries from the parent's
// answer are cached shallowly (children: null) so rows render before their own fetch.
function fetchNode(id) {
  const cached = nodes.get(id);
  if (cached && cached.path !== undefined) return Promise.resolve(cached);
  if (pending.has(id)) return pending.get(id);
  const started = generation;
  const promise = getJSON(`/api/node/${id}`)
    .then((node) => {
      if (started !== generation) throw new Error("the tree changed");
      nodes.set(id, node);
      for (const child of node.children) {
        if (!nodes.has(child.id)) nodes.set(child.id, { ...child, children: null });
      }
      return node;
    })
    .finally(() => pending.delete(id));
  pending.set(id, promise);
  return promise;
}

let messageTimer = null;

// Shown in place of the status for a few seconds, or until the scan state changes.
function showMessage(text, isError) {
  clearTimeout(messageTimer);
  store.message = text ? { text, isError, state: store.status && store.status.state } : null;
  if (text) {
    messageTimer = setTimeout(() => {
      store.message = null;
      renderStatus();
    }, 8000);
  }
  renderStatus();
}

// Status line.

function renderStatus() {
  const s = store.status;
  el.status.textContent = "";
  if (!s) return;
  const path = document.createElement("span");
  path.className = "path";
  path.textContent = s.path || "";
  const add = (text, cls) => {
    const span = document.createElement("span");
    if (cls) span.className = cls;
    span.textContent = text;
    el.status.appendChild(span);
  };
  if (store.message && store.message.state === s.state) {
    add(store.message.text, store.message.isError ? "error" : "");
    return;
  }
  switch (s.state) {
    case "idle":
      add("No scan yet.");
      break;
    case "scanning":
      add("Scanning ");
      el.status.appendChild(path);
      add(`: ${formatCount(s.dirs)} folders, ${formatCount(s.files)} files, ${formatBytes(s.bytes)}, ${formatElapsed(s.elapsed_ms)}`);
      break;
    case "done": {
      el.status.appendChild(path);
      add(`: ${formatCount(s.dirs)} folders, ${formatCount(s.files)} files, ${formatBytes(s.bytes)} in ${formatElapsed(s.elapsed_ms)}`);
      if (s.error_count > 0) {
        const button = document.createElement("button");
        button.type = "button";
        button.textContent = `${formatCount(s.error_count)} unreadable`;
        button.title = "List the folders the scan could not read";
        button.addEventListener("click", showErrors);
        el.status.appendChild(button);
      }
      break;
    }
    case "failed":
      el.status.appendChild(path);
      add(`: ${s.error || "scan failed"}`, "error");
      break;
    default:
      add(s.state);
  }
}

function renderControls() {
  const s = store.status;
  const scanning = s && s.state === "scanning";
  const loaded = store.loadedPath !== null;
  el.scan.disabled = Boolean(scanning);
  el.rescan.disabled = Boolean(scanning) || !(s && s.path);
  const selected = store.selection !== null ? nodes.get(store.selection) : null;
  el.open.disabled = !loaded || !selected;
  el.zoom.disabled = !loaded || !selected || selected.kind !== "dir" || store.selection === store.root;
  el.root.disabled = !loaded || store.root === 0;
  for (const button of el.mode.querySelectorAll("button")) {
    button.setAttribute("aria-checked", String(button.dataset.mode === store.mode));
  }
}

async function showErrors() {
  try {
    const data = await getJSON("/api/errors");
    el.errorsTitle.textContent = `${formatCount(data.count)} unreadable ${data.count === 1 ? "folder" : "folders"}`;
    el.errorRows.textContent = "";
    for (const item of data.items) {
      const tr = document.createElement("tr");
      const path = document.createElement("td");
      path.className = "name";
      path.textContent = item.path;
      const error = document.createElement("td");
      error.className = "name muted";
      error.textContent = item.error;
      tr.append(path, error);
      el.errorRows.appendChild(tr);
    }
    if (data.count > data.items.length) {
      const tr = document.createElement("tr");
      const td = document.createElement("td");
      td.colSpan = 2;
      td.className = "muted";
      td.textContent = `and ${formatCount(data.count - data.items.length)} more`;
      tr.appendChild(td);
      el.errorRows.appendChild(tr);
    }
    el.errors.showModal();
  } catch (e) {
    showMessage(e.message, true);
  }
}

// Directory list.

function renderList() {
  const rows = flatten(nodes, expanded, 0);
  const fragment = document.createDocumentFragment();
  for (const row of rows) {
    fragment.appendChild(renderRow(row));
  }
  el.rows.textContent = "";
  el.rows.appendChild(fragment);
}

function renderRow({ id, depth, node, parent, path }) {
  const mode = store.mode;
  const bytes = bytesOf(node, mode);
  const isDir = node.kind === "dir";
  const tr = document.createElement("tr");
  tr.dataset.id = id;
  tr.className = node.kind + (id === store.selection ? " selected" : "");
  tr.setAttribute("role", "row");
  tr.setAttribute("aria-level", String(depth + 1));
  if (isDir && node.has_children) tr.setAttribute("aria-expanded", String(expanded.has(id)));
  tr.title = `${path}\n${formatCount(bytes)} bytes${node.hard_link ? " (hard link)" : ""}`;

  const name = document.createElement("td");
  name.className = "name";
  name.style.paddingLeft = `${4 + depth * 16}px`;
  const twisty = document.createElement("span");
  twisty.className = "twisty";
  twisty.textContent = isDir && node.has_children ? (expanded.has(id) ? "▾" : "▸") : "";
  const icon = document.createElement("span");
  icon.className = `icon ${node.kind}`;
  const label = document.createElement("span");
  label.className = "label";
  label.textContent = node.name;
  name.append(twisty, icon, label);
  if (node.kind === "symlink") {
    const mark = document.createElement("span");
    mark.className = "mark";
    mark.textContent = "symlink";
    name.appendChild(mark);
  }
  if (node.hard_link) {
    const mark = document.createElement("span");
    mark.className = "mark";
    mark.textContent = "⇄";
    mark.title = "hard link: counted under its first path";
    name.appendChild(mark);
  }

  const size = document.createElement("td");
  size.className = "num";
  size.textContent = formatBytes(bytes);
  size.title = node.hard_link ? `hard link, ${formatCount(bytes)} bytes` : `${formatCount(bytes)} bytes`;

  const pct = document.createElement("td");
  pct.className = "pct";
  const share = parent ? percent(bytes, bytesOf(parent, mode)) : 100;
  const bar = document.createElement("span");
  bar.className = "bar";
  bar.style.setProperty("--pct", `${share.toFixed(2)}%`);
  bar.textContent = formatPercent(share);
  pct.appendChild(bar);

  const items = document.createElement("td");
  items.className = "num";
  const files = document.createElement("td");
  files.className = "num";
  const dirs = document.createElement("td");
  dirs.className = "num";
  if (isDir) {
    items.textContent = formatCount(node.files + node.dirs);
    files.textContent = formatCount(node.files);
    dirs.textContent = formatCount(node.dirs);
  }

  const date = document.createElement("td");
  date.className = "date";
  date.textContent = formatDate(node.mtime);

  tr.append(name, size, pct, items, files, dirs, date);
  return tr;
}

function rowOf(id) {
  return el.rows.querySelector(`tr[data-id="${id}"]`);
}

async function expand(id) {
  const node = nodes.get(id);
  if (!node || node.kind !== "dir" || !node.has_children) return;
  expanded.add(id);
  if (!Array.isArray(node.children)) {
    try {
      await fetchNode(id);
    } catch (e) {
      expanded.delete(id);
      showMessage(e.message, true);
      return;
    }
  }
  renderList();
}

function collapse(id) {
  if (!expanded.has(id)) return;
  expanded.delete(id);
  renderList();
}

function toggle(id) {
  if (expanded.has(id)) collapse(id);
  else expand(id);
}

function select(id, { scroll = false } = {}) {
  if (store.selection !== id) {
    const previous = store.selection !== null ? rowOf(store.selection) : null;
    if (previous) previous.classList.remove("selected");
    store.selection = id;
    const row = id !== null ? rowOf(id) : null;
    if (row) row.classList.add("selected");
    drawTreemap();
  }
  if (scroll && id !== null) {
    const row = rowOf(id);
    if (row) row.scrollIntoView({ block: "nearest" });
  }
  renderControls();
}

// Expands every ancestor of `id` and scrolls its row into view.
async function reveal(id) {
  try {
    const node = await fetchNode(id);
    const chain = [];
    let parent = node.parent;
    while (parent !== null && parent !== undefined) {
      const ancestor = await fetchNode(parent);
      chain.push(parent);
      parent = ancestor.parent;
    }
    for (const ancestor of chain) expanded.add(ancestor);
    renderList();
    select(id, { scroll: true });
  } catch (e) {
    showMessage(e.message, true);
  }
}

async function openInFinder(id) {
  if (id === null || id === undefined || store.loadedPath === null) return;
  try {
    await postJSON("/api/open", { id });
  } catch (e) {
    showMessage(e.message, true);
  }
}

el.rows.addEventListener("click", (event) => {
  const tr = event.target.closest("tr");
  if (!tr) return;
  const id = Number(tr.dataset.id);
  if (event.target.classList.contains("twisty")) {
    toggle(id);
    return;
  }
  select(id);
});

el.rows.addEventListener("dblclick", (event) => {
  const tr = event.target.closest("tr");
  if (!tr || event.target.classList.contains("twisty")) return;
  openInFinder(Number(tr.dataset.id));
});

el.list.addEventListener("keydown", (event) => {
  if (store.loadedPath === null) return;
  const rows = flatten(nodes, expanded, 0);
  if (rows.length === 0) return;
  const index = rows.findIndex((r) => r.id === store.selection);
  const move = (to) => {
    const target = rows[Math.max(0, Math.min(rows.length - 1, to))];
    select(target.id, { scroll: true });
  };
  switch (event.key) {
    case "ArrowDown":
      move(index < 0 ? 0 : index + 1);
      break;
    case "ArrowUp":
      move(index < 0 ? 0 : index - 1);
      break;
    case "ArrowRight": {
      if (index < 0) return move(0);
      const row = rows[index];
      if (row.node.kind === "dir" && row.node.has_children) {
        if (expanded.has(row.id)) move(index + 1);
        else expand(row.id);
      }
      break;
    }
    case "ArrowLeft": {
      if (index < 0) return move(0);
      const row = rows[index];
      if (expanded.has(row.id)) collapse(row.id);
      else if (row.node.parent !== null && row.node.parent !== undefined) select(row.node.parent, { scroll: true });
      break;
    }
    case "Enter":
      if (index >= 0) openInFinder(rows[index].id);
      break;
    case "Home":
      move(0);
      break;
    case "End":
      move(rows.length - 1);
      break;
    default:
      return;
  }
  event.preventDefault();
});

// Treemap.

const ctx = el.canvas.getContext("2d");
let layoutRequest = 0;
let resizeTimer = null;
let hovered = null;

function paneSize() {
  const rect = el.pane.getBoundingClientRect();
  return { w: Math.max(1, Math.floor(rect.width)), h: Math.max(1, Math.floor(rect.height)) };
}

async function fetchTreemap() {
  if (store.loadedPath === null) {
    store.layout = null;
    drawTreemap();
    return;
  }
  const { w, h } = paneSize();
  const request = ++layoutRequest;
  try {
    const layout = await getJSON(`/api/treemap?root=${store.root}&w=${w}&h=${h}&min=1&mode=${store.mode}`);
    if (request !== layoutRequest) return;
    store.layout = layout;
    drawTreemap();
  } catch (e) {
    if (request === layoutRequest) showMessage(e.message, true);
  }
}

function scheduleTreemap() {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(fetchTreemap, 150);
}

const gradients = new Map();

function cushion(color, h) {
  const key = `${color}|${Math.ceil(h)}`;
  let gradient = gradients.get(key);
  if (!gradient) {
    gradient = ctx.createLinearGradient(0, 0, 0, Math.ceil(h));
    gradient.addColorStop(0, shade(color, 0.28));
    gradient.addColorStop(0.5, color);
    gradient.addColorStop(1, shade(color, -0.3));
    if (gradients.size > 4000) gradients.clear();
    gradients.set(key, gradient);
  }
  return gradient;
}

function drawTreemap() {
  const { w, h } = paneSize();
  const dpr = window.devicePixelRatio || 1;
  const style = getComputedStyle(document.documentElement);
  const panel = style.getPropertyValue("--panel").trim();
  const accent = style.getPropertyValue("--accent").trim();
  const outline = style.getPropertyValue("--tm-outline").trim();
  const border = style.getPropertyValue("--tm-border").trim();

  el.canvas.width = Math.round(w * dpr);
  el.canvas.height = Math.round(h * dpr);
  el.canvas.style.width = `${w}px`;
  el.canvas.style.height = `${h}px`;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.fillStyle = panel;
  ctx.fillRect(0, 0, w, h);

  const layout = store.layout;
  if (!layout) return;

  const highlight = store.highlightExt;
  ctx.lineWidth = 1;
  ctx.strokeStyle = border;
  for (const r of layout.rects) {
    const dim = highlight !== null && (r.kind === "dir" || r.ext !== highlight);
    ctx.globalAlpha = dim ? 0.3 : 1;
    ctx.save();
    ctx.translate(r.x, r.y);
    ctx.fillStyle = cushion(r.color, r.h);
    ctx.fillRect(0, 0, r.w, r.h);
    if (r.w > 3 && r.h > 3) ctx.strokeRect(0.5, 0.5, r.w - 1, r.h - 1);
    ctx.restore();
  }
  ctx.globalAlpha = 1;

  ctx.strokeStyle = outline;
  for (const d of layout.dirs) {
    if (d.w * d.h > 400) ctx.strokeRect(Math.round(d.x) + 0.5, Math.round(d.y) + 0.5, Math.round(d.w) - 1, Math.round(d.h) - 1);
  }

  const selected = selectionBounds(layout);
  if (selected) {
    ctx.lineWidth = 2;
    ctx.strokeStyle = accent;
    ctx.strokeRect(selected.x + 1, selected.y + 1, Math.max(0, selected.w - 2), Math.max(0, selected.h - 2));
  }
}

function selectionBounds(layout) {
  const id = store.selection;
  if (id === null) return null;
  return layout.dirs.find((d) => d.id === id) || layout.rects.find((r) => r.id === id) || null;
}

function hitTest(x, y) {
  const layout = store.layout;
  if (!layout) return null;
  for (let i = layout.rects.length - 1; i >= 0; i--) {
    const r = layout.rects[i];
    if (x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) return r;
  }
  return null;
}

function canvasPoint(event) {
  const bounds = el.canvas.getBoundingClientRect();
  return { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
}

function hideTooltip() {
  el.tooltip.hidden = true;
  hovered = null;
}

function placeTooltip(event) {
  const margin = 12;
  const tip = el.tooltip;
  let x = event.clientX + margin;
  let y = event.clientY + margin;
  const tw = tip.offsetWidth;
  const th = tip.offsetHeight;
  if (x + tw > window.innerWidth - 4) x = Math.max(4, event.clientX - tw - margin);
  if (y + th > window.innerHeight - 4) y = Math.max(4, event.clientY - th - margin);
  tip.style.left = `${x}px`;
  tip.style.top = `${y}px`;
}

el.canvas.addEventListener("mousemove", async (event) => {
  const { x, y } = canvasPoint(event);
  const rect = hitTest(x, y);
  if (!rect) {
    hideTooltip();
    return;
  }
  if (hovered !== rect.id) {
    hovered = rect.id;
    try {
      const node = await fetchNode(rect.id);
      if (hovered !== rect.id) return;
      el.tooltip.textContent = "";
      const path = document.createElement("span");
      path.className = "path";
      const inner = document.createElement("span");
      inner.textContent = node.path;
      path.appendChild(inner);
      const size = document.createElement("span");
      size.className = "size";
      const bytes = bytesOf(node, store.mode);
      size.textContent = `${formatBytes(bytes)} (${formatCount(bytes)} bytes)`;
      if (node.kind === "dir") size.textContent += `, ${formatCount(node.files)} files`;
      el.tooltip.append(path, size);
      el.tooltip.hidden = false;
    } catch (e) {
      hideTooltip();
      return;
    }
  }
  if (!el.tooltip.hidden) placeTooltip(event);
});

el.canvas.addEventListener("mouseleave", hideTooltip);

el.canvas.addEventListener("click", (event) => {
  const { x, y } = canvasPoint(event);
  const rect = hitTest(x, y);
  if (rect) reveal(rect.id);
});

el.canvas.addEventListener("dblclick", (event) => {
  const { x, y } = canvasPoint(event);
  const rect = hitTest(x, y);
  if (rect) openInFinder(rect.id);
});

new ResizeObserver(() => {
  drawTreemap();
  scheduleTreemap();
}).observe(el.pane);

if (window.matchMedia) {
  window.matchMedia("(prefers-color-scheme: dark)").addEventListener("change", drawTreemap);
}

function renderCaption() {
  const node = nodes.get(store.root);
  const text = node && node.path ? node.path : store.loadedPath || "";
  el.caption.textContent = "";
  const span = document.createElement("span");
  span.textContent = text;
  el.caption.appendChild(span);
  el.caption.title = text;
}

// Root and mode changes.

async function setRoot(id) {
  if (id === store.root) return;
  store.root = id;
  store.highlightExt = null;
  try {
    await fetchNode(id);
  } catch (e) {
    showMessage(e.message, true);
  }
  renderCaption();
  renderControls();
  fetchTreemap();
  loadBottom();
}

function setMode(mode) {
  if (mode === store.mode) return;
  store.mode = mode;
  renderControls();
  renderList();
  fetchTreemap();
  loadBottom();
}

el.mode.addEventListener("click", (event) => {
  const button = event.target.closest("button[data-mode]");
  if (button) setMode(button.dataset.mode);
});

el.zoom.addEventListener("click", () => {
  const node = store.selection !== null ? nodes.get(store.selection) : null;
  if (node && node.kind === "dir") setRoot(store.selection);
});

el.root.addEventListener("click", () => setRoot(0));

el.open.addEventListener("click", () => openInFinder(store.selection));

// Bottom panel: extensions and largest files.

let bottomRequest = 0;

async function loadBottom() {
  const request = ++bottomRequest;
  if (store.loadedPath === null) {
    el.extRows.textContent = "";
    el.largestRows.textContent = "";
    return;
  }
  try {
    const [extensions, largest] = await Promise.all([
      getJSON(`/api/extensions?root=${store.root}&mode=${store.mode}`),
      getJSON(`/api/largest?root=${store.root}&n=200&mode=${store.mode}`),
    ]);
    if (request !== bottomRequest) return;
    renderExtensions(extensions);
    renderLargest(largest);
  } catch (e) {
    if (request === bottomRequest) showMessage(e.message, true);
  }
}

function renderExtensions(data) {
  el.extRows.textContent = "";
  const mode = store.mode;
  for (const item of data.items) {
    const bytes = mode === "physical" ? item.physical : item.bytes;
    const tr = document.createElement("tr");
    tr.dataset.ext = item.ext;
    if (item.ext === store.highlightExt) tr.classList.add("highlight");
    tr.title = `${formatCount(bytes)} bytes`;

    const ext = document.createElement("td");
    ext.className = "name";
    const swatch = document.createElement("span");
    swatch.className = "swatch";
    swatch.style.background = item.color;
    ext.appendChild(swatch);
    const label = document.createElement("span");
    label.textContent = item.ext === "" ? "(no extension)" : `.${item.ext}`;
    if (item.ext === "") label.className = "muted";
    ext.appendChild(label);

    const size = document.createElement("td");
    size.className = "num";
    size.textContent = formatBytes(bytes);

    const pct = document.createElement("td");
    pct.className = "pct";
    const share = percent(bytes, data.total);
    const bar = document.createElement("span");
    bar.className = "bar";
    bar.style.setProperty("--pct", `${share.toFixed(2)}%`);
    bar.textContent = formatPercent(share);
    pct.appendChild(bar);

    const files = document.createElement("td");
    files.className = "num";
    files.textContent = formatCount(item.files);

    tr.append(ext, size, pct, files);
    el.extRows.appendChild(tr);
  }
  renderHint();
}

function renderHint() {
  el.hint.textContent = store.highlightExt === null
    ? ""
    : `Highlighting ${store.highlightExt === "" ? "files without an extension" : `.${store.highlightExt}`} (Escape clears)`;
}

function setHighlight(ext) {
  store.highlightExt = ext;
  for (const tr of el.extRows.children) {
    tr.classList.toggle("highlight", ext !== null && tr.dataset.ext === ext);
  }
  renderHint();
  drawTreemap();
}

el.extRows.addEventListener("click", (event) => {
  const tr = event.target.closest("tr");
  if (!tr) return;
  const ext = tr.dataset.ext;
  setHighlight(store.highlightExt === ext ? null : ext);
});

function renderLargest(data) {
  el.largestRows.textContent = "";
  const mode = store.mode;
  for (const item of data.items) {
    const bytes = mode === "physical" ? item.physical : item.size;
    const tr = document.createElement("tr");
    tr.dataset.id = item.id;
    if (item.id === store.selection) tr.classList.add("selected");
    tr.title = `${item.path}\n${formatCount(bytes)} bytes\n${formatDate(item.mtime)}`;
    const size = document.createElement("td");
    size.className = "num";
    size.textContent = formatBytes(bytes);
    const path = document.createElement("td");
    path.className = "name";
    const inner = document.createElement("span");
    inner.textContent = item.path;
    path.appendChild(inner);
    tr.append(size, path);
    el.largestRows.appendChild(tr);
  }
}

el.largestRows.addEventListener("click", (event) => {
  const tr = event.target.closest("tr");
  if (!tr) return;
  for (const row of el.largestRows.children) row.classList.toggle("selected", row === tr);
  reveal(Number(tr.dataset.id));
});

el.largestRows.addEventListener("dblclick", (event) => {
  const tr = event.target.closest("tr");
  if (tr) openInFinder(Number(tr.dataset.id));
});

for (const tab of document.querySelectorAll('.tabs [role="tab"]')) {
  tab.addEventListener("click", () => {
    for (const other of document.querySelectorAll('.tabs [role="tab"]')) {
      const on = other === tab;
      other.setAttribute("aria-selected", String(on));
      document.getElementById(other.getAttribute("aria-controls")).hidden = !on;
    }
  });
}

// Scanning.

el.scanForm.addEventListener("submit", (event) => {
  event.preventDefault();
  startScan(el.path.value.trim());
});

el.rescan.addEventListener("click", () => {
  if (store.status && store.status.path) startScan(store.status.path);
});

async function startScan(path) {
  if (!path) {
    showMessage("Type a folder path to scan.", true);
    return;
  }
  try {
    await postJSON("/api/scan", { path });
  } catch (e) {
    showMessage(e.message, true);
  }
}

// Forgets the loaded tree and loads the new one once a scan has finished.
async function loadTree() {
  generation += 1;
  nodes.clear();
  expanded.clear();
  pending.clear();
  store.selection = null;
  store.root = 0;
  store.highlightExt = null;
  store.layout = null;
  store.loadedPath = store.status.path;
  hideTooltip();
  try {
    await fetchNode(0);
  } catch (e) {
    store.loadedPath = null;
    showMessage(e.message, true);
    return;
  }
  expanded.add(0);
  store.selection = 0;
  renderList();
  renderCaption();
  renderControls();
  renderHint();
  fetchTreemap();
  loadBottom();
}

function clearTree() {
  generation += 1;
  nodes.clear();
  pending.clear();
  expanded.clear();
  store.selection = null;
  store.root = 0;
  store.highlightExt = null;
  store.layout = null;
  store.loadedPath = null;
  hideTooltip();
  renderList();
  renderCaption();
  renderControls();
  renderHint();
  drawTreemap();
  loadBottom();
}

function applyStatus(status) {
  const previous = store.status;
  store.status = status;
  if (store.message && store.message.state !== status.state) store.message = null;
  if (status.path && (!previous || previous.path !== status.path || el.path.value === "")) {
    el.path.value = status.path;
  }
  renderStatus();
  renderControls();
  const becameDone = status.state === "done" && (!previous || previous.state !== "done" || store.loadedPath === null);
  if (becameDone) {
    loadTree();
  } else if (status.state === "scanning" && store.loadedPath !== null) {
    clearTree();
  } else if (status.state === "failed" && store.loadedPath !== null) {
    clearTree();
  }
}

function connectEvents() {
  const source = new EventSource("/api/events");
  source.addEventListener("status", (event) => {
    try {
      applyStatus(JSON.parse(event.data));
    } catch (e) {
      showMessage(`Bad status event: ${e.message}`, true);
    }
  });
  source.addEventListener("error", () => {
    if (source.readyState === EventSource.CLOSED) {
      showMessage("Connection to the server lost", true);
    }
  });
}

// Global keys and splitters.

document.addEventListener("keydown", (event) => {
  if (event.key !== "Escape") return;
  if (el.errors.open) return;
  if (store.highlightExt !== null) {
    setHighlight(null);
    event.preventDefault();
  }
});

el.errorsClose.addEventListener("click", () => el.errors.close());
el.errors.addEventListener("click", (event) => {
  if (event.target === el.errors) el.errors.close();
});

function splitter(handle, cssVar, axis) {
  handle.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    document.body.classList.add("dragging", axis === "x" ? "dragging-v" : "dragging-h");
    const onMove = (move) => {
      const root = document.documentElement;
      if (axis === "x") {
        const width = Math.max(160, Math.min(window.innerWidth - 200, move.clientX));
        root.style.setProperty(cssVar, `${width}px`);
      } else {
        const height = Math.max(60, Math.min(window.innerHeight - 160, window.innerHeight - move.clientY));
        root.style.setProperty(cssVar, `${height}px`);
      }
    };
    const onUp = () => {
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      handle.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("dragging", "dragging-v", "dragging-h");
    };
    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
    handle.addEventListener("pointercancel", onUp);
  });
}

splitter($("vsplit"), "--list-w", "x");
splitter($("hsplit"), "--bottom-h", "y");

// Start.

getJSON("/api/status")
  .then(applyStatus)
  .catch((e) => showMessage(e.message, true))
  .finally(connectEvents);
