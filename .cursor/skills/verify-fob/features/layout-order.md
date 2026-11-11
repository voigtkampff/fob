# Column and card order

Process columns (all but the name column) and the Dashboard and Processes
summary cards can be dragged into a new order. The dragged item lifts and
follows the pointer while the others slide into the order it would land in
(a dragged column's body cells come with its header); release saves once,
Escape puts everything back. Alt+Arrow moves the focused header or card one
place. Slides are WAAPI animations started only by a change in order, and
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
- Alt+Right on a focused header moves it and keeps focus;
- a hidden column, a reload and tree view keep the saved order, and tree view
  shows every family row with the long name cut and the deepest name inside
  its cell;
- dragging a Processes card by its title, or a Dashboard card by its head
  (across a row), makes room live, opens nothing, leaves no animation running
  after the drop, and the next click opens its details;
- both card orders survive a reload; saved orders with unknown, repeated and
  missing ids still show every column and card once;
- at 1920 px the name column is at most its widest, a long name is cut with a
  tooltip, the other columns share the rest evenly (within 1 px) and the
  actions column ends at the right edge; at 1024 px the name column is at its
  narrowest and the table scrolls;
- with reduced motion a drag still lifts the item but nothing slides.

It writes `<lang>-1920-*.png`, `<lang>-1024-*.png` and `ui-reorder.json`.
`scripts/layout-order.test.mjs` covers the order helpers, the card ids the
settings file accepts, and how `columnWidths` shares spare width. Mocked readings in a
browser only; real drags in WebView2, display scaling and the packaged
settings file need a Windows run.
