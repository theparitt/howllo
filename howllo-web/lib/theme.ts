import type { CSSProperties } from "react";

export function hexToRgba(hex: string, alpha: number): string {
  const normalized = hex.replace("#", "");
  if (normalized.length !== 6) {
    return `rgba(243, 105, 73, ${alpha})`;
  }

  const red = Number.parseInt(normalized.slice(0, 2), 16);
  const green = Number.parseInt(normalized.slice(2, 4), 16);
  const blue = Number.parseInt(normalized.slice(4, 6), 16);
  return `rgba(${red}, ${green}, ${blue}, ${alpha})`;
}

export function themedSurfaceStyle(color: string | null | undefined): CSSProperties | undefined {
  if (!color) return undefined;

  return {
    background: `linear-gradient(180deg, ${hexToRgba(color, 0.42)}, rgba(255, 255, 255, 0.92))`,
    borderColor: hexToRgba(color, 0.28),
  };
}
