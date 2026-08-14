import type { RemotePromptImage } from "./protocol";

export const MAX_COMPOSER_ATTACHMENTS = 8;
export const MAX_COMPOSER_IMAGE_BYTES = 10 * 1024 * 1024;

export interface BrowserComposerAttachment extends RemotePromptImage {
  id: string;
  previewUrl: string;
  sizeBytes: number;
}

export interface ComposerAttachmentResult {
  attachments: BrowserComposerAttachment[];
  error?: string;
}

function readFileAsDataUrl(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.addEventListener("load", () =>
      typeof reader.result === "string" ? resolve(reader.result) : reject(new Error("Invalid file")),
    );
    reader.addEventListener("error", () => reject(reader.error ?? new Error("Unable to read file")));
    reader.readAsDataURL(file);
  });
}

export async function readComposerImageFiles(
  files: FileList | readonly File[],
  existingCount: number,
  makeId: () => string = () => crypto.randomUUID(),
): Promise<ComposerAttachmentResult> {
  const remaining = Math.max(0, MAX_COMPOSER_ATTACHMENTS - existingCount);
  if (remaining === 0) {
    return {
      attachments: [],
      error: `You can attach up to ${MAX_COMPOSER_ATTACHMENTS} images per message.`,
    };
  }

  const selected = Array.from(files);
  const attachments: BrowserComposerAttachment[] = [];
  let error =
    selected.length > remaining
      ? `You can attach up to ${MAX_COMPOSER_ATTACHMENTS} images per message.`
      : undefined;

  for (const file of selected.slice(0, remaining)) {
    if (!file.type.toLowerCase().startsWith("image/")) {
      error = `'${file.name}' is not a supported image.`;
      continue;
    }
    if (file.size <= 0 || file.size > MAX_COMPOSER_IMAGE_BYTES) {
      error = `'${file.name}' exceeds the 10 MB attachment limit.`;
      continue;
    }
    try {
      const previewUrl = await readFileAsDataUrl(file);
      const data = previewUrl.split(",", 2)[1] ?? "";
      if (!data) {
        error = `Failed to read '${file.name}'.`;
        continue;
      }
      attachments.push({
        id: makeId(),
        name: file.name || "image",
        mimeType: file.type.toLowerCase(),
        data,
        previewUrl,
        sizeBytes: file.size,
      });
    } catch {
      error = `Failed to read '${file.name}'.`;
    }
  }

  return { attachments, ...(error ? { error } : {}) };
}

export function toRemotePromptImages(
  attachments: readonly BrowserComposerAttachment[],
): RemotePromptImage[] {
  return attachments.map(({ name, mimeType, data }) => ({ name, mimeType, data }));
}
