// Dragging files between the two panes. Uses pointer events, because the window's native
// file drop (from Explorer/Finder) switches off HTML drag and drop in the webview on Windows.

export const drag = $state({
  active: false,
  from: null as "left" | "right" | null,
  count: 0,
  x: 0,
  y: 0,
  over: null as "left" | "right" | null,
  move: false,
});

let onDrop: ((from: "left" | "right", to: "left" | "right", move: boolean) => void) | null = null;

export function setDropHandler(fn: typeof onDrop) {
  onDrop = fn;
}

function paneAt(x: number, y: number): "left" | "right" | null {
  return (document.elementFromPoint(x, y)?.closest("[data-pane]")?.getAttribute("data-pane") as "left" | "right" | null) ?? null;
}

/** Call on pointerdown on a selected row; the drag starts after the pointer moves a few pixels. */
export function beginDrag(e: PointerEvent, from: "left" | "right", count: number) {
  if (e.button !== 0) return;
  const sx = e.clientX;
  const sy = e.clientY;
  const move = (ev: PointerEvent) => {
    if (!drag.active && Math.hypot(ev.clientX - sx, ev.clientY - sy) < 6) return;
    drag.active = true;
    drag.from = from;
    drag.count = count;
    drag.x = ev.clientX;
    drag.y = ev.clientY;
    drag.move = ev.shiftKey;
    drag.over = paneAt(ev.clientX, ev.clientY);
  };
  const up = (ev: PointerEvent) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    if (drag.active) {
      const to = paneAt(ev.clientX, ev.clientY);
      if (to && to !== from) onDrop?.(from, to, ev.shiftKey);
    }
    drag.active = false;
    drag.over = null;
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}
