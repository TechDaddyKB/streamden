/** StreamDen design tokens. Single source: `tokens.json`; `tokens.css` is generated from it (`pnpm tokens:css`). */
import data from "./tokens.json";

export const tokens = data;
export type Theme = keyof typeof data.color.themes;
export type ColorRole = keyof (typeof data.color.themes)["dark"];

type Refs<T> = { [K in keyof T]: T[K] extends string ? string : Refs<T[K]> };

const toVars = (node: object, path: string[] = []): unknown =>
  Object.fromEntries(
    Object.entries(node).map(([k, v]) => [
      k,
      typeof v === "string"
        ? `var(--sd-${[...path, k].map((p) => p.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)).join("-")})`
        : toVars(v, [...path, k]),
    ]),
  );

/** `var(--sd-…)` references for use in inline styles, e.g. `vars.color.bg`. Colour and shadow follow the active theme. */
export const vars = {
  color: toVars(data.color.themes.dark, ["color"]) as Refs<(typeof data.color.themes)["dark"]>,
  font: toVars(data.font, ["font"]) as Refs<typeof data.font>,
  text: toVars(data.text, ["text"]) as Refs<typeof data.text>,
  space: toVars(data.space, ["space"]) as Refs<typeof data.space>,
  radius: toVars(data.radius, ["radius"]) as Refs<typeof data.radius>,
  shadow: toVars(data.shadow.dark, ["shadow"]) as Refs<typeof data.shadow.dark>,
  motion: toVars(data.motion, ["motion"]) as Refs<typeof data.motion>,
  focusRing: "var(--sd-focus-ring)",
};
