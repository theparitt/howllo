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

export function contrastInk(hex: string): string {
  const match = /^#([0-9a-f]{6})$/i.exec(hex);
  if (!match) return "#ffffff";
  const channels = [0, 2, 4].map((offset) => Number.parseInt(match[1]!.slice(offset, offset + 2), 16) / 255);
  const [red, green, blue] = channels.map((value) => value <= 0.04045 ? value / 12.92 : ((value + 0.055) / 1.055) ** 2.4);
  const luminance = 0.2126 * red! + 0.7152 * green! + 0.0722 * blue!;
  return luminance > 0.179 ? "#17252c" : "#ffffff";
}

export function themedSurfaceStyle(color: string | null | undefined): CSSProperties | undefined {
  if (!color) return undefined;

  return {
    background: `linear-gradient(180deg, ${hexToRgba(color, 0.42)}, rgba(255, 255, 255, 0.92))`,
    borderColor: hexToRgba(color, 0.28),
  };
}
