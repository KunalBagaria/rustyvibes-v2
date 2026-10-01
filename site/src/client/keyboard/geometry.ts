import { BufferGeometry, ExtrudeGeometry, Float32BufferAttribute, LatheGeometry, Shape, Path, Vector2 } from "three";
import { RoundedBoxGeometry } from "three/addons/geometries/RoundedBoxGeometry.js";
import { mergeVertices } from "three/addons/utils/BufferGeometryUtils.js";
import { ATLAS_HEIGHT, ATLAS_WIDTH, PX } from "./atlas";
import type { PlacedKey } from "./layout";

/** Keycap proportions in key units (1u = 19.05 mm). */
export const KEY = { gap: 0.06, height: 0.44, inset: 0.095, dish: 0.035, radius: 0.075, travel: 0.11 } as const;
/** Case proportions in key units. */
export const CASE = { border: 0.62, height: 1.0, rim: 0.2, radius: 0.36, well: 0.1 } as const;

const smoothstep = (a: number, b: number, t: number) => {
  const x = Math.min(1, Math.max(0, (t - a) / (b - a)));
  return x * x * (3 - 2 * x);
};

const caps = new Map<number, BufferGeometry>();

/** A tapered, dished keycap `w` units wide, its base at y = 0. Shared per width. */
export function keycapGeometry(w: number): BufferGeometry {
  const cached = caps.get(w);
  if (cached) return cached;
  const fw = w - KEY.gap;
  const fd = 1 - KEY.gap;
  const h = KEY.height;
  let geo: BufferGeometry = new RoundedBoxGeometry(fw, h, fd, 4, KEY.radius);
  geo.deleteAttribute("normal");
  geo.deleteAttribute("uv");
  geo = mergeVertices(geo, 1e-4);
  const pos = geo.getAttribute("position");
  const topHalf = fw / 2 - KEY.inset;
  for (let i = 0; i < pos.count; i++) {
    let x = pos.getX(i);
    let y = pos.getY(i);
    let z = pos.getZ(i);
    const t = (y + h / 2) / h;
    x *= 1 - (t * 2 * KEY.inset) / fw;
    z *= 1 - (t * 2 * KEY.inset) / fd;
    const across = Math.min(1, Math.abs(x) / topHalf);
    y -= KEY.dish * (1 - across * across) * smoothstep(0.8, 1, t);
    pos.setXYZ(i, x, y + h / 2, z);
  }
  geo.computeVertexNormals();
  caps.set(w, geo);
  return geo;
}

/** A copy of the cap geometry whose UVs point at the key's cell in the legend atlas. */
export function keyGeometry(key: PlacedKey): BufferGeometry {
  const geo = keycapGeometry(key.def.w).clone();
  const pos = geo.getAttribute("position");
  const uv = new Float32Array(pos.count * 2);
  const cx = key.x + key.def.w / 2;
  const cz = key.y + 0.5;
  for (let i = 0; i < pos.count; i++) {
    uv[i * 2] = ((cx + pos.getX(i)) * PX) / ATLAS_WIDTH;
    uv[i * 2 + 1] = ((cz + pos.getZ(i)) * PX) / ATLAS_HEIGHT;
  }
  geo.setAttribute("uv", new Float32BufferAttribute(uv, 2));
  return geo;
}

function roundedRect(target: Shape | Path, x: number, y: number, w: number, h: number, r: number): void {
  target.moveTo(x + r, y);
  target.lineTo(x + w - r, y);
  target.quadraticCurveTo(x + w, y, x + w, y + r);
  target.lineTo(x + w, y + h - r);
  target.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
  target.lineTo(x + r, y + h);
  target.quadraticCurveTo(x, y + h, x, y + h - r);
  target.lineTo(x, y + r);
  target.quadraticCurveTo(x, y, x + r, y);
}

/** The anodised frame around a `fieldW` × `fieldD` key field; its rim sits at y = CASE.rim. */
export function frameGeometry(fieldW: number, fieldD: number): BufferGeometry {
  const w = fieldW + 2 * CASE.border;
  const d = fieldD + 2 * CASE.border;
  const outer = new Shape();
  roundedRect(outer, -w / 2, -d / 2, w, d, CASE.radius);
  const hole = new Path();
  roundedRect(hole, -fieldW / 2 - CASE.well, -fieldD / 2 - CASE.well, fieldW + 2 * CASE.well, fieldD + 2 * CASE.well, 0.14);
  outer.holes.push(hole);
  const bevel = 0.07;
  const geo = new ExtrudeGeometry(outer, {
    depth: CASE.height - 2 * bevel,
    bevelEnabled: true,
    bevelThickness: bevel,
    bevelSize: bevel,
    bevelSegments: 5,
    curveSegments: 14,
  });
  geo.rotateX(-Math.PI / 2);
  geo.translate(0, CASE.rim - CASE.height + bevel, 0);
  return geo;
}

/** A chamfered rotary knob, its base at y = 0. */
export function knobGeometry(): BufferGeometry {
  const points = [
    new Vector2(0, 0),
    new Vector2(0.34, 0),
    new Vector2(0.355, 0.015),
    new Vector2(0.355, 0.3),
    new Vector2(0.33, 0.335),
    new Vector2(0, 0.335),
  ];
  return new LatheGeometry(points, 64);
}
