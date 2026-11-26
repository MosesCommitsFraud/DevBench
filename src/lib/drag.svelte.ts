// Dragging files inside the app with pointer events: the window's native file drop (from
// Explorer/Finder) switches off HTML drag and drop in the webview on Windows.
// Drop targets are elements with a `data-drop` attribute; its value names the target.

export const drag = $state({
  active: false,
  count: 0,
  x: 0,
  y: 0,
  /** data-drop value under the pointer, or null. */
  over: null as string | null,
  move: false,
});

export function dropTargetAt(x: number, y: number): string | null {
  return (document.elementFromPoint(x, y)?.closest("[data-drop]") as HTMLElement | null)?.dataset.drop ?? null;
}

/**
 * Call on pointerdown on a selected row; the drag starts after the pointer moves a few pixels.
 * `accept` says whether a target takes the drop; `onDrop` gets the target and whether Shift was held.
 */
export function beginDrag(
  e: PointerEvent,
  count: number,
  accept: (target: string) => boolean,
  onDrop: (target: string, shift: boolean) => void,
) {
  if (e.button !== 0) return;
  const sx = e.clientX;
  const sy = e.clientY;
  const move = (ev: PointerEvent) => {
    if (!drag.active && Math.hypot(ev.clientX - sx, ev.clientY - sy) < 6) return;
    drag.active = true;
    drag.count = count;
    drag.x = ev.clientX;
    drag.y = ev.clientY;
    drag.move = ev.shiftKey;
    const t = dropTargetAt(ev.clientX, ev.clientY);
    drag.over = t && accept(t) ? t : null;
  };
  const up = (ev: PointerEvent) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    if (drag.active) {
      const t = dropTargetAt(ev.clientX, ev.clientY);
      if (t && accept(t)) onDrop(t, ev.shiftKey);
    }
    drag.active = false;
    drag.over = null;
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}
