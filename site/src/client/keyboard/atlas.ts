import type { Colourway } from "./colorways";
import { FIELD, type PlacedKey } from "./layout";

/** Atlas pixels per key unit. */
export const PX = 160;
export const ATLAS_WIDTH = FIELD.width * PX;
export const ATLAS_HEIGHT = 1024;

const FAMILY = '-apple-system, BlinkMacSystemFont, "Inter", "Helvetica Neue", Arial, sans-serif';

/** Where a keycap's top face sits inside its cell (the cap is tapered and gapped). */
const FACE = { side: 0.17, top: 0.15, bottom: 0.18 };

function capColour(key: PlacedKey, c: Colourway): [fill: string, ink: string] {
  if (key.def.role === "accent") return [c.accent, c.accentLegend];
  if (key.def.role === "mod") return [c.mod, c.modLegend];
  return [c.alpha, c.legend];
}

/**
 * Draws every key's cell: cap colour edge to edge, legend on the top face. The canvas
 * is laid out like a top view of the keyboard, so each key's UVs are just its position.
 */
export function drawAtlas(canvas: HTMLCanvasElement, keys: PlacedKey[], colourway: Colourway): void {
  canvas.width = ATLAS_WIDTH;
  canvas.height = ATLAS_HEIGHT;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("no 2D canvas");
  ctx.fillStyle = colourway.plate;
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  for (const key of keys) {
    if (key.def.role === "knob") continue;
    const [fill, ink] = capColour(key, colourway);
    const x = key.x * PX;
    const y = key.y * PX;
    const w = key.def.w * PX;
    ctx.fillStyle = fill;
    ctx.fillRect(x, y, w, PX);
    ctx.fillStyle = ink;
    const left = x + FACE.side * PX;
    const right = x + w - FACE.side * PX;
    const top = y + FACE.top * PX;
    const bottom = y + PX - FACE.bottom * PX;
    const cx = x + w / 2;
    const legend = key.def.legend;
    switch (legend.kind) {
      case "letter":
        ctx.font = `500 ${0.27 * PX}px ${FAMILY}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(legend.text, cx, y + 0.47 * PX);
        break;
      case "stack":
        ctx.font = `500 ${0.19 * PX}px ${FAMILY}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(legend.top, cx, y + 0.36 * PX);
        ctx.fillText(legend.bottom, cx, y + 0.6 * PX);
        break;
      case "small":
        ctx.font = `500 ${0.14 * PX}px ${FAMILY}`;
        ctx.textAlign = "center";
        ctx.textBaseline = "middle";
        ctx.fillText(legend.text, cx, y + 0.5 * PX);
        break;
      case "mod": {
        const atLeft = legend.align === "left";
        ctx.textAlign = atLeft ? "left" : "right";
        const edge = atLeft ? left : right;
        if (legend.symbol) {
          ctx.font = `400 ${0.19 * PX}px ${FAMILY}`;
          ctx.textBaseline = "top";
          ctx.fillText(legend.symbol, edge, top);
        }
        ctx.font = `500 ${0.12 * PX}px ${FAMILY}`;
        ctx.textBaseline = "alphabetic";
        ctx.fillText(legend.word, edge, bottom);
        break;
      }
      case "none":
        break;
    }
  }
}

/** Waits briefly for web fonts so legends never draw in a fallback face. */
export async function fontsReady(timeoutMs = 1000): Promise<void> {
  await Promise.race([document.fonts?.ready, new Promise((resolve) => setTimeout(resolve, timeoutMs))]);
}
