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

/** Cream echoes the app icon's keycap; all three share the rust accent. */
export const COLOURWAYS = {
  cream: {
    alpha: "#efe6d2",
    mod: "#d8ccb1",
    accent: "#e8592b",
    legend: "#4b4338",
    modLegend: "#4b4338",
    accentLegend: "#fff3e6",
    frame: "#2c2d31",
    frameRoughness: 0.42,
    plate: "#1e1e21",
    knob: "#3b3c41",
  },
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
  silver: {
    alpha: "#f6f6f8",
    mod: "#dcdde1",
    accent: "#e8592b",
    legend: "#3a3a3c",
    modLegend: "#3a3a3c",
    accentLegend: "#fff3e6",
    frame: "#c9ccd1",
    frameRoughness: 0.3,
    plate: "#7c8189",
    knob: "#dcdfe3",
  },
} as const satisfies Record<string, Colourway>;

export type ColourwayName = keyof typeof COLOURWAYS;
