# 05: The page

**What to build:** the browser page: top bar, directory list, treemap canvas,
extension list and largest files tabs, linked selection, open in Finder, logical or
physical toggle, progress while scanning, system theme. Vanilla JS, one canvas, no
external resources.

Spec sections: Solution, Implementation decisions (Page, API), user stories 3 to 9,
13 to 26, 35, 37, 40.

Shape:
- `app.js` has a small store `{ status, selection, root, mode, highlightExt }` and
  three renderers that read it; `EventSource('/api/events')` updates `status`; the
  list fetches `/api/node/{id}` lazily and caches; the treemap fetches
  `/api/treemap` for `root`, the canvas size (device pixel ratio aware) and `mode`,
  redraws on resize (debounced 150 ms); the bottom panel fetches `/api/extensions`
  and `/api/largest?n=200`.
- Selecting from the treemap expands the list down to the node: fetch the node, then
  its ancestors by walking `parent` from `/api/node`, expanding each, then scroll the
  row into view.
- A selected directory is outlined in the treemap from the `dirs` bounds; a selected
  file's rect is outlined. Clicking an extension dims every rect of another colour.
- Zoom to selection sets `root` to the selected directory; a Root button resets it.
- Double-click on a row or a rect, Enter on a row, and the Open in Finder button POST
  `/api/open` with `Content-Type: application/json`.
- Sizes: `formatBytes` with one decimal (B, KB, MB, GB, TB, base 1000 like Finder);
  the `title` attribute carries exact bytes. Dates as local `YYYY-MM-DD HH:MM`.
- Keyboard in the list: Up/Down move, Right expands, Left collapses or goes to the
  parent, Enter opens in Finder.
- A light cushion shade on each rect (a vertical gradient from lighter to darker)
  and 1 px borders around directories above 20 px.
- The scan status text shows folders, files, bytes and elapsed time while scanning
  and the totals plus error count when done; the error count links to a dialog
  listing `/api/errors`.
- No element, menu item or key does anything destructive. The page never calls a
  delete endpoint, because there is none.

**Blocked by:** 04.

**Status:** done

- [ ] `cargo run -- . --no-open` then open the URL by hand: the three views appear and
      match the figures from `/api/node/0` (record the check in the report)
- [ ] `formatBytes` and the list's expand/collapse bookkeeping are pure functions at
      the top of `app.js`, and a `node --test` file is not required; the report lists
      the manual checks done for stories 3 to 9, 13 to 26, 35, 37 and 40
- [ ] the page loads with no console errors and no request leaves 127.0.0.1
- [ ] no text, button or menu mentions delete, trash, remove or move
- [ ] `cargo test` still green (the server embeds the files; the status test still finds
      `macdirtree` in the HTML)
