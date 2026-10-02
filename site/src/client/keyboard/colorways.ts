export interface Colourway {
  alpha: string;
  mod: string;
  accent: string;
  legend: string;
  modLegend: string;
  accentLegend: string;
  frame: string;
  frameRoughness: number;
  plate: string;
  knob: string;
}

/** Graphite is the default; all three share the rust brand colour. */
export const COLOURWAYS = {
  graphite: {
    alpha: "#3b3d42",
    mod: "#2a2c30",
    accent: "#e8592b",
    legend: "#ececee",
    modLegend: "#c7c7cc",
    accentLegend: "#fff3e6",
    frame: "#1b1c1f",
    frameRoughness: 0.38,
    plate: "#141416",
    knob: "#2c2d31",
  },
  titanium: {
    alpha: "#4a4947",
    mod: "#393836",
    accent: "#e8592b",
    legend: "#edece9",
    modLegend: "#cfcdc8",
    accentLegend: "#fff3e6",
    frame: "#8e8a83",
    frameRoughness: 0.28,
    plate: "#282725",
    knob: "#a39e96",
  },
  stealth: {
    alpha: "#222224",
    mod: "#222224",
    accent: "#e8592b",
    legend: "#ff8a4c",
    modLegend: "#ff8a4c",
    accentLegend: "#1c1c1e",
    frame: "#131315",
    frameRoughness: 0.34,
    plate: "#0b0b0c",
    knob: "#1d1d20",
  },
} as const satisfies Record<string, Colourway>;

export type ColourwayName = keyof typeof COLOURWAYS;
