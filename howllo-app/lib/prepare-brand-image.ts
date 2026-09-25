const IMAGE_TYPES = new Set(["image/png", "image/jpeg", "image/webp"]);
const MAX_SOURCE_BYTES = 5 * 1024 * 1024;
const MAX_SOURCE_SIDE = 4096;

/** Fit a raster image inside the requested box without changing its aspect ratio. */
export async function prepareBrandImage(file: File, maxWidth: number, maxHeight: number): Promise<File> {
  if (!IMAGE_TYPES.has(file.type)) throw new Error("Choose a PNG, JPG or WebP image.");
  if (file.size > MAX_SOURCE_BYTES) throw new Error("Choose an image smaller than 5 MB.");

  const objectUrl = URL.createObjectURL(file);
  try {
    const image = await new Promise<HTMLImageElement>((resolve, reject) => {
      const element = new Image();
      element.onload = () => resolve(element);
      element.onerror = () => reject(new Error("This image could not be opened."));
      element.src = objectUrl;
    });
    const { naturalWidth: width, naturalHeight: height } = image;
    if (!width || !height || width > MAX_SOURCE_SIDE || height > MAX_SOURCE_SIDE) {
      throw new Error("Choose an image no larger than 4096 × 4096 pixels.");
    }
    const scale = Math.min(1, maxWidth / width, maxHeight / height);
    const canvas = document.createElement("canvas");
    canvas.width = Math.max(1, Math.round(width * scale));
    canvas.height = Math.max(1, Math.round(height * scale));
    const context = canvas.getContext("2d");
    if (!context) throw new Error("Your browser could not prepare this image.");
    context.imageSmoothingEnabled = true;
    context.imageSmoothingQuality = "high";
    context.drawImage(image, 0, 0, canvas.width, canvas.height);
    const blob = await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((result) => result ? resolve(result) : reject(new Error("Could not prepare this image.")), "image/png");
    });
    return new File([blob], "brand-image.png", { type: "image/png" });
  } finally {
    URL.revokeObjectURL(objectUrl);
  }
}
