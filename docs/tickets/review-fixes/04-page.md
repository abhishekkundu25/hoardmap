# 04: Page: stale answers, keyboard, collapsible Rebuildable tab

**What to build:** spec Solutions 5 and 6. Request counters on every async write that
can be overtaken (treemap after a tree reset, `reveal()`, the live-refresh catch
handlers); keyboard access to the "more" row, the extension rows and the Largest files
rows; the path field keeps trailing spaces; Rebuildable groups collapsed by default,
caution visible when collapsed, 50 folders then "Show all N", open state kept across
refreshes for the current root, disclosure control separate from the highlight click.

**Blocked by:** none.

**Status:** done

## Acceptance

- [ ] A treemap, `reveal()` or live-refresh answer from an older request never
      changes the screen; each counter named in the report.
- [ ] Keyboard only: reach the 201st child of a folder, highlight an extension,
      select and open a Largest files entry.
- [ ] A path ending in a space is sent as typed.
- [ ] Rebuildable: groups start collapsed; caution lines show when collapsed; the why
      line, docs address and folders show when expanded; 50 folders then "Show all N";
      open groups stay open during a scan's refreshes; Enter or Space on the
      disclosure control toggles it; the group row click still highlights.
- [ ] Light and dark, existing colour tokens only.
- [ ] `cargo build` and `timeout 120 cargo test --test api --test resources` green;
      screenshots of the Rebuildable tab collapsed and expanded in the debug app.
