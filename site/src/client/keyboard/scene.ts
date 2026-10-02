import {
  BoxGeometry,
  CanvasTexture,
  DirectionalLight,
  Group,
  LinearMipmapLinearFilter,
  MathUtils,
  Mesh,
  MeshStandardMaterial,
  NeutralToneMapping,
  PerspectiveCamera,
  PMREMGenerator,
  Raycaster,
  Scene,
  SRGBColorSpace,
  Vector2,
  Vector3,
  WebGLRenderer,
} from "three";
import { RoomEnvironment } from "three/addons/environments/RoomEnvironment.js";
import { drawAtlas, fontsReady } from "./atlas";
import { COLOURWAYS, type ColourwayName } from "./colorways";
import { CASE, frameGeometry, KEY, keyGeometry, knobGeometry } from "./geometry";
import { FIELD, placedKeys, type PlacedKey } from "./layout";

export interface KeyboardOptions {
  colourway: ColourwayName;
  reducedMotion: boolean;
  /** Called if WebGL goes away; the page falls back to its poster. */
  onLost?: () => void;
}

export interface KeyboardView {
  press(code: string): void;
  release(code: string): void;
  releaseAll(): void;
  setColourway(name: ColourwayName): void;
  /** 0 while the stage is fully in view, rising to 1 as it scrolls away. */
  setScroll(progress: number): void;
  onKey(listener: (code: string, down: boolean) => void): void;
}

interface KeyState {
  key: PlacedKey;
  mesh: Mesh;
  y: number;
  v: number;
  target: number;
}

const FOV = 24;
const ELEVATION = MathUtils.degToRad(34);
const SPRING_K = 1100;
const SPRING_C = 2 * Math.sqrt(SPRING_K) * 0.62;
const PARALLAX = { yaw: 0.07, pitch: 0.035 };

/** Gives the main thread back between heavy setup steps. */
const yieldToMain = () =>
  new Promise<void>((resolve) => {
    const scheduler = (globalThis as { scheduler?: { yield?: () => Promise<void> } }).scheduler;
    if (scheduler?.yield) void scheduler.yield().then(resolve);
    else setTimeout(resolve, 0);
  });

export async function createKeyboard(canvas: HTMLCanvasElement, opts: KeyboardOptions): Promise<KeyboardView> {
  const renderer = new WebGLRenderer({ canvas, antialias: true, alpha: true, powerPreference: "high-performance" });
  renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
  renderer.setClearColor(0x000000, 0);
  renderer.toneMapping = NeutralToneMapping;
  renderer.toneMappingExposure = 1.05;

  await yieldToMain();
  const scene = new Scene();
  const pmrem = new PMREMGenerator(renderer);
  scene.environment = pmrem.fromScene(new RoomEnvironment(), 0.04).texture;
  scene.environmentIntensity = 0.7;
  pmrem.dispose();

  const keyLight = new DirectionalLight(0xfff0de, 2.6);
  keyLight.position.set(-6, 12, 8);
  const rimLight = new DirectionalLight(0xd4e4ff, 1.4);
  rimLight.position.set(8, 5, -10);
  const fillLight = new DirectionalLight(0xffffff, 0.3);
  fillLight.position.set(0, 3, 12);
  scene.add(keyLight, rimLight, fillLight);

  let colourway = COLOURWAYS[opts.colourway];
  const keys = placedKeys();
  await fontsReady();
  await yieldToMain();
  const atlasCanvas = document.createElement("canvas");
  drawAtlas(atlasCanvas, keys, colourway);
  const atlas = new CanvasTexture(atlasCanvas);
  atlas.colorSpace = SRGBColorSpace;
  atlas.flipY = false;
  atlas.minFilter = LinearMipmapLinearFilter;
  atlas.anisotropy = renderer.capabilities.getMaxAnisotropy();

  await yieldToMain();
  const capMaterial = new MeshStandardMaterial({ map: atlas, roughness: 0.62, metalness: 0 });
  const frameMaterial = new MeshStandardMaterial({ color: colourway.frame, metalness: 0.9, roughness: colourway.frameRoughness });
  const plateMaterial = new MeshStandardMaterial({ color: colourway.plate, metalness: 0.3, roughness: 0.7 });
  const knobMaterial = new MeshStandardMaterial({ color: colourway.knob, metalness: 1, roughness: 0.24 });

  const board = new Group();
  board.add(new Mesh(frameGeometry(FIELD.width, FIELD.depth), frameMaterial));
  const plate = new Mesh(
    new BoxGeometry(FIELD.width + 2 * CASE.well, 0.05, FIELD.depth + 2 * CASE.well),
    plateMaterial,
  );
  plate.position.y = -0.025;
  board.add(plate);

  const states = new Map<string, KeyState>();
  const meshes: Mesh[] = [];
  for (const key of keys) {
    const x = key.x + key.def.w / 2 - FIELD.width / 2;
    const z = key.y + 0.5 - FIELD.depth / 2;
    if (key.def.role === "knob") {
      const knob = new Mesh(knobGeometry(), knobMaterial);
      knob.position.set(x, 0, z);
      board.add(knob);
      continue;
    }
    const mesh = new Mesh(keyGeometry(key), capMaterial);
    mesh.position.set(x, 0, z);
    mesh.userData.code = key.def.code;
    board.add(mesh);
    meshes.push(mesh);
    if (key.def.code) states.set(key.def.code, { key, mesh, y: 0, v: 0, target: 0 });
  }
  scene.add(board);
  await yieldToMain();

  const camera = new PerspectiveCamera(FOV, 2, 0.1, 200);
  const target = new Vector3(0, -0.1, 0.25);

  const frameCamera = () => {
    const width = canvas.clientWidth || 1;
    const height = canvas.clientHeight || 1;
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    const halfW = ((FIELD.width + 2 * CASE.border) / 2) * 1.04;
    const halfD = (FIELD.depth + 2 * CASE.border) / 2;
    const tanV = Math.tan(MathUtils.degToRad(FOV / 2));
    const fitWidth = halfW / (tanV * camera.aspect);
    const fitDepth = ((halfD * Math.sin(ELEVATION) + 0.6) / tanV) * 1.08;
    const distance = Math.max(fitWidth, fitDepth) + halfD * Math.cos(ELEVATION);
    camera.position.set(target.x, target.y + Math.sin(ELEVATION) * distance, target.z + Math.cos(ELEVATION) * distance);
    camera.lookAt(target);
    camera.updateProjectionMatrix();
  };
  frameCamera();

  // Render on demand: one frame per request, more only while something still moves.
  const active = new Set<KeyState>();
  const parallax = { yaw: 0, pitch: 0, toYaw: 0, toPitch: 0 };
  const scroll = { now: 0, to: 0 };
  let scheduled = false;
  let last = 0;
  let lost = false;

  const step = (dt: number): boolean => {
    for (const s of active) {
      const a = SPRING_K * (s.target - s.y) - SPRING_C * s.v;
      s.v += a * dt;
      s.y += s.v * dt;
      if (Math.abs(s.target - s.y) < 1e-4 && Math.abs(s.v) < 1e-3) {
        s.y = s.target;
        s.v = 0;
        active.delete(s);
      }
      s.mesh.position.y = s.y;
    }
    const ease = 1 - Math.exp(-dt * 6);
    parallax.yaw += (parallax.toYaw - parallax.yaw) * ease;
    parallax.pitch += (parallax.toPitch - parallax.pitch) * ease;
    scroll.now += (scroll.to - scroll.now) * (1 - Math.exp(-dt * 10));
    board.rotation.y = parallax.yaw;
    board.rotation.x = parallax.pitch - scroll.now * 0.32;
    board.position.y = -scroll.now * 0.7;
    const moving =
      Math.abs(parallax.toYaw - parallax.yaw) > 1e-4 ||
      Math.abs(parallax.toPitch - parallax.pitch) > 1e-4 ||
      Math.abs(scroll.to - scroll.now) > 1e-4;
    return active.size > 0 || moving;
  };

  const tick = (now: number) => {
    scheduled = false;
    if (lost) return;
    const dt = last ? Math.min((now - last) / 1000, 1 / 30) : 1 / 60;
    last = now;
    const busy = step(dt);
    renderer.render(scene, camera);
    if (busy) requestRender();
    else last = 0;
  };

  function requestRender() {
    if (scheduled || lost) return;
    scheduled = true;
    requestAnimationFrame(tick);
  }

  const setTarget = (code: string, down: boolean) => {
    const s = states.get(code);
    if (!s) return;
    s.target = down ? -KEY.travel : 0;
    active.add(s);
    requestRender();
  };

  const listeners = new Set<(code: string, down: boolean) => void>();
  const raycaster = new Raycaster();
  const ndc = new Vector2();
  const pick = (e: PointerEvent): string | undefined => {
    const rect = canvas.getBoundingClientRect();
    ndc.set(((e.clientX - rect.left) / rect.width) * 2 - 1, -((e.clientY - rect.top) / rect.height) * 2 + 1);
    raycaster.setFromCamera(ndc, camera);
    return raycaster.intersectObjects(meshes, false)[0]?.object.userData.code as string | undefined;
  };
  const pointers = new Map<number, string>();
  canvas.addEventListener("pointerdown", (e) => {
    const code = pick(e);
    if (!code) return;
    canvas.setPointerCapture(e.pointerId);
    pointers.set(e.pointerId, code);
    setTarget(code, true);
    for (const l of listeners) l(code, true);
  });
  const lift = (e: PointerEvent) => {
    const code = pointers.get(e.pointerId);
    if (!code) return;
    pointers.delete(e.pointerId);
    setTarget(code, false);
    for (const l of listeners) l(code, false);
  };
  canvas.addEventListener("pointerup", lift);
  canvas.addEventListener("pointercancel", lift);

  let inView = true;
  new IntersectionObserver(([entry]) => {
    inView = entry?.isIntersecting ?? true;
  }).observe(canvas);
  const finePointer = window.matchMedia("(pointer: fine)").matches;
  if (finePointer && !opts.reducedMotion) {
    window.addEventListener(
      "pointermove",
      (e) => {
        if (!inView) return;
        parallax.toYaw = (e.clientX / window.innerWidth - 0.5) * 2 * PARALLAX.yaw;
        parallax.toPitch = (e.clientY / window.innerHeight - 0.5) * 2 * PARALLAX.pitch;
        canvas.style.cursor = pick(e) ? "pointer" : "";
        requestRender();
      },
      { passive: true },
    );
  }

  new ResizeObserver(() => {
    frameCamera();
    requestRender();
  }).observe(canvas);

  canvas.addEventListener("webglcontextlost", (e) => {
    e.preventDefault();
    lost = true;
    opts.onLost?.();
  });

  await renderer.compileAsync(scene, camera);
  await yieldToMain();
  renderer.render(scene, camera);

  return {
    press: (code) => setTarget(code, true),
    release: (code) => setTarget(code, false),
    releaseAll: () => {
      for (const [code, s] of states) if (s.target !== 0) setTarget(code, false);
    },
    setColourway(name) {
      colourway = COLOURWAYS[name];
      drawAtlas(atlasCanvas, keys, colourway);
      atlas.needsUpdate = true;
      frameMaterial.color.set(colourway.frame);
      frameMaterial.roughness = colourway.frameRoughness;
      plateMaterial.color.set(colourway.plate);
      knobMaterial.color.set(colourway.knob);
      requestRender();
    },
    setScroll(progress) {
      if (opts.reducedMotion) return;
      scroll.to = Math.min(1, Math.max(0, progress));
      requestRender();
    },
    onKey(listener) {
      listeners.add(listener);
    },
  };
}
