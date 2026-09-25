"use client";

import { useEffect, useRef, useState } from "react";

const MAX_FILE_BYTES = 8 * 1024 * 1024;
const MAX_SOURCE_SIDE = 4096;
const AVATAR_SIDE = 512;
const ALLOWED_TYPES = new Set(["image/jpeg", "image/png", "image/webp"]);

type Source = { image: HTMLImageElement; name: string };

type AvatarCropperProps = {
  disabled: boolean;
  onCrop: (file: File) => Promise<void>;
  onError: (message: string) => void;
};

function drawCrop(canvas: HTMLCanvasElement, source: Source, zoom: number, x: number, y: number) {
  const { image } = source;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("Your browser could not prepare the avatar image.");
  const visibleSide = Math.min(image.naturalWidth, image.naturalHeight) / zoom;
  const sx = (image.naturalWidth - visibleSide) * (x / 100);
  const sy = (image.naturalHeight - visibleSide) * (y / 100);
  context.clearRect(0, 0, AVATAR_SIDE, AVATAR_SIDE);
  context.imageSmoothingEnabled = true;
  context.imageSmoothingQuality = "high";
  context.drawImage(image, sx, sy, visibleSide, visibleSide, 0, 0, AVATAR_SIDE, AVATAR_SIDE);
}

export function AvatarCropper({ disabled, onCrop, onError }: AvatarCropperProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [source, setSource] = useState<Source | null>(null);
  const [zoom, setZoom] = useState(1);
  const [x, setX] = useState(50);
  const [y, setY] = useState(50);

  useEffect(() => {
    if (source && canvas.current) drawCrop(canvas.current, source, zoom, x, y);
  }, [source, zoom, x, y]);

  function chooseFile(event: React.ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (!file) return;
    onError("");
    setSource(null);
    if (!ALLOWED_TYPES.has(file.type)) {
      onError("Choose a JPG, PNG, or WebP image.");
      return;
    }
    if (file.size > MAX_FILE_BYTES) {
      onError("Choose an image smaller than 8 MB.");
      return;
    }
    const url = URL.createObjectURL(file);
    const image = new Image();
    image.onload = () => {
      URL.revokeObjectURL(url);
      if (!image.naturalWidth || !image.naturalHeight || image.naturalWidth > MAX_SOURCE_SIDE || image.naturalHeight > MAX_SOURCE_SIDE) {
        onError("Choose an image no larger than 4096 × 4096 pixels.");
        return;
      }
      setZoom(1);
      setX(50);
      setY(50);
      setSource({ image, name: file.name });
    };
    image.onerror = () => {
      URL.revokeObjectURL(url);
      onError("This image could not be opened.");
    };
    image.src = url;
  }

  async function applyCrop() {
    if (!source || !canvas.current || disabled) return;
    try {
      drawCrop(canvas.current, source, zoom, x, y);
      const blob = await new Promise<Blob>((resolve, reject) => {
        canvas.current!.toBlob((result) => result ? resolve(result) : reject(new Error("Could not create the cropped avatar.")), "image/jpeg", 0.9);
      });
      await onCrop(new File([blob], "avatar.jpg", { type: "image/jpeg" }));
      setSource(null);
    } catch (cause) {
      onError(cause instanceof Error ? cause.message : "Could not crop the avatar.");
    }
  }

  return (
    <div className="avatar-cropper">
      <label className="field-label">
        Avatar image
        <input className="field" type="file" accept="image/jpeg,image/png,image/webp" onChange={chooseFile} disabled={disabled} />
      </label>
      <p className="section-subtitle">JPG, PNG, or WebP · up to 8 MB and 4096 × 4096 pixels. Saved as 512 × 512.</p>
      {source ? <div className="avatar-cropper__editor">
        <canvas ref={canvas} width={AVATAR_SIDE} height={AVATAR_SIDE} className="avatar-cropper__preview" role="img" aria-label="Square avatar crop preview" />
        <span className="section-subtitle">Crop {source.name}</span>
        <label className="field-label">Zoom<input type="range" min="1" max="3" step="0.1" value={zoom} onChange={(event) => setZoom(Number(event.target.value))} disabled={disabled} /></label>
        <label className="field-label">Move left or right<input type="range" min="0" max="100" value={x} onChange={(event) => setX(Number(event.target.value))} disabled={disabled} /></label>
        <label className="field-label">Move up or down<input type="range" min="0" max="100" value={y} onChange={(event) => setY(Number(event.target.value))} disabled={disabled} /></label>
        <div className="toolbar">
          <button className="button button--cta" type="button" onClick={() => void applyCrop()} disabled={disabled}>Use this avatar</button>
          <button className="button" type="button" onClick={() => setSource(null)} disabled={disabled}>Cancel</button>
        </div>
      </div> : null}
    </div>
  );
}
