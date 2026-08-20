import { clsx, type ClassValue } from "clsx"
import { twMerge } from "tailwind-merge"

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

/** One decoded cursor image, as returned by the `parse_cur` / `parse_ani` commands. */
export interface CurFrame {
  width: number;
  height: number;
  hotspot_x: number;
  hotspot_y: number;
  rgba: number[];
}

/** The 32x32 image if the cursor has one, else the largest available. */
export function bestFrame(frames: CurFrame[]): CurFrame | null {
  if (!frames.length) return null;
  return (
    frames.find((f) => f.width === 32 && f.height === 32) ??
    frames.reduce((a, b) => (a.width * a.height >= b.width * b.height ? a : b))
  );
}

/** Raw RGBA -> PNG data URL, via an offscreen canvas. */
export function frameToDataUrl(frame: CurFrame): string {
  const canvas = document.createElement("canvas");
  canvas.width = frame.width;
  canvas.height = frame.height;
  const ctx = canvas.getContext("2d")!;
  ctx.putImageData(
    new ImageData(new Uint8ClampedArray(frame.rgba), frame.width, frame.height),
    0, 0,
  );
  return canvas.toDataURL("image/png");
}

/** A CSS `cursor` value that renders `frame`, honouring an overridden hotspot. */
export function frameToCssCursor(frame: CurFrame, hotspot?: { x: number; y: number }): string {
  const hx = hotspot?.x ?? frame.hotspot_x;
  const hy = hotspot?.y ?? frame.hotspot_y;
  return `url('${frameToDataUrl(frame)}') ${hx} ${hy}, auto`;
}
