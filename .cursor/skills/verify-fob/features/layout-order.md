# Column and card order

Process columns (all but the name column) and the Dashboard and Processes
summary cards can be dragged into a new order. The dragged item lifts and
follows the pointer while the others slide into the order it would land in
(a dragged column's body cells come with its header); release saves once,
Escape puts everything back. There is no keyboard shortcut for reordering.
Slides are WAAPI animations started only by a change in order, and
reduced motion turns them off (`utils/drag-reorder.js`). Orders are saved as `behavior.columnOrder`,
`behavior.dashboardCards` and `behavior.processCards` (empty means the
default order). The settings file keeps only distinct known ids
(`preferences.rs`), and the page puts back any missing id next to its default
neighbour (`utils/layout-order.js`), so a bad file never hides or repeats a
card or column.

```bash
npm run build
node .cursor/skills/verify-fob/bin/ui-reorder.mjs /workspace/scratch/verify/evidence/ui-reorder
```

Drives the real main window route with generic mocked processes (a long name
and a three level family) in headless Chrome with real pointer and key events,
zh and en, at 1920 and 1024 px. It fails unless:

- while the CPU header is held over PID, CPU and its body cells are lifted
  under the pointer, PID and the rest already stand in the landing order with
  body cells under their headers, and nothing is saved yet;
- releasing saves CPU before PID once, without sorting;
- a drop on the name column keeps it first; a drop on the actions column puts
  the column last before actions;
- Escape mid drag (after the neighbours moved), released over the same
  header, puts the order back and sorts nothing;
- a press that moves under 6 px still sorts;
- Alt+Right on a focused header or card moves and saves nothing and keeps
  focus;
- a hidden column, a reload and tree view keep the saved order, and tree view
  shows every family row with the long name cut and the deepest name inside
  its cell;
- dragging a Processes card by its title, or a Dashboard card by its head
  (across a row), makes room live, opens nothing, leaves no animation running
  after the drop, and the next click opens its details;
- both card orders survive a reload; saved orders with unknown, repeated and
  missing ids still show every column and card once;
- at 1920 px the name column is 460 px (560 px in tree view), a long name is
  cut with a tooltip, every data column keeps its own content width
  (`columnWidth` in `utils/table-layout.js`, within 1 px; also with PID
  hidden and in tree view), the room left over stays empty before the
  actions column, which ends at the right edge, and no header label is cut;
  at 1024 px the name column is at its narrowest, nothing is left empty and
  the table scrolls;
- dragging a column's right edge (an 8 px zone with no visible mark until
  hovered) widens only that column while the other columns, the order, the
  sort and the rows stay put; nothing is saved until release, then exactly
  once; Escape mid drag puts the width back and saves nothing; the width
  survives a reload, a column reorder, hiding and showing the column, tree
  view and a 1024 px window; a double-click on the edge drops the saved width
  and the column returns to its automatic width;
- with reduced motion a drag still lifts the item but nothing slides;
- every summary card shows a dot grip whose left edge is within 1 px of the
  value below it, fully opaque on hover, in dark and light themes; column
  headers show no grip and no hover explanation; the grip takes no
  focus and a Dashboard card drags from it;
- no column draws a vertical line between columns, in the header or rows
  (the actions column keeps its own edge);
- the 用户 (User) column shows SYSTEM, LOCAL SERVICE and a user name in full
  and an owner with no account as its SID, whole in the tooltip;
- a dragged card is a copy in the top layer (its own slot stays empty).

`ui-drag-clip.mjs <evidence-dir>` drags a Processes card, a Dashboard card
and a column header toward each window edge at 1280 and 1920 px (and with
the Dashboard scrolled). A lifted card must be drawn in full, inside the
window, with no ancestor cutting it off; Escape must put it down. A header
stays inside the table's own scroll area by design.

It writes `<lang>-1920-*.png`, `<lang>-1024-*.png` and `ui-reorder.json`.
`scripts/layout-order.test.mjs` covers the order helpers, the card ids the
settings file accepts, how `columnLayout` fits the columns (own widths,
the name and other long text columns growing to a cap, the rest empty) and
how widths set by hand win and are cleaned when read back. Mocked readings in a
browser only; real drags in WebView2, display scaling and the packaged
settings file need a Windows run.
