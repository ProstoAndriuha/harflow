import { useState, type ChangeEvent, type DragEvent } from 'react';
import { useI18n } from '../i18n/index.js';

const DEFAULT_MAX_HAR_BYTES = 100 * 1024 * 1024;

async function readFileBytes(file: File): Promise<Uint8Array> {
  if (typeof file.arrayBuffer === 'function') {
    const buf = await file.arrayBuffer();
    return new Uint8Array(buf);
  }
  return new Promise<Uint8Array>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => {
      if (reader.result instanceof ArrayBuffer) {
        resolve(new Uint8Array(reader.result));
      } else if (typeof reader.result === 'string') {
        resolve(new TextEncoder().encode(reader.result));
      } else {
        reject(new Error('Failed to read HAR file bytes'));
      }
    };
    reader.onerror = () => reject(new Error('Failed to read HAR file'));
    reader.readAsArrayBuffer(file);
  });
}

function isGzipBytes(fileName: string, bytes: Uint8Array): boolean {
  return (
    fileName.toLowerCase().endsWith('.gz') ||
    (bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b)
  );
}

async function decodeHarBytes(
  fileName: string,
  bytes: Uint8Array,
  maxBytes: number,
): Promise<string> {
  if (!isGzipBytes(fileName, bytes)) {
    if (bytes.byteLength > maxBytes) {
      throw new Error(
        `File '${fileName}' exceeds ${Math.round(maxBytes / (1024 * 1024))} MB limit`,
      );
    }
    return new TextDecoder().decode(bytes);
  }

  if (typeof DecompressionStream === 'undefined') {
    throw new Error('Gzip decompression is not supported in this browser environment');
  }

  const stream = new Blob([bytes as BlobPart])
    .stream()
    .pipeThrough(new DecompressionStream('gzip'));
  const reader = stream.getReader();
  const chunks: Uint8Array[] = [];
  let total = 0;

  while (true) {
    const { done, value } = await reader.read();
    if (done) {
      break;
    }
    if (value) {
      total += value.byteLength;
      if (total > maxBytes) {
        await reader.cancel();
        throw new Error(
          `Decompressed '${fileName}' exceeds ${Math.round(maxBytes / (1024 * 1024))} MB safety cap`,
        );
      }
      chunks.push(value);
    }
  }

  const merged = new Uint8Array(total);
  let offset = 0;
  for (const c of chunks) {
    merged.set(c, offset);
    offset += c.byteLength;
  }
  return new TextDecoder().decode(merged);
}

interface HarDropzoneProps {
  maxBytes?: number;
  onImportFiles: (files: Array<{ sourceName: string; rawText: string }>) => Promise<void>;
}

export function HarDropzone({
  maxBytes = DEFAULT_MAX_HAR_BYTES,
  onImportFiles,
}: HarDropzoneProps) {
  const { t } = useI18n();
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState(false);
  const [localError, setLocalError] = useState<string | null>(null);

  const processFileList = async (fileList: FileList | File[]) => {
    const files = Array.from(fileList);
    if (files.length === 0) {
      return;
    }
    setBusy(true);
    setLocalError(null);
    try {
      const decoded: Array<{ sourceName: string; rawText: string }> = [];
      for (const file of files) {
        const bytes = await readFileBytes(file);
        const rawText = await decodeHarBytes(file.name, bytes, maxBytes);
        decoded.push({ sourceName: file.name, rawText });
      }
      await onImportFiles(decoded);
    } catch (err: unknown) {
      setLocalError(err instanceof Error ? err.message : 'Failed to decode HAR file');
    } finally {
      setBusy(false);
    }
  };

  const handleChange = (e: ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      void processFileList(e.target.files);
      e.target.value = '';
    }
  };

  const handleDragOver = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setDragging(true);
  };

  const handleDragLeave = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setDragging(false);
  };

  const handleDrop = (e: DragEvent<HTMLDivElement>) => {
    e.preventDefault();
    setDragging(false);
    if (e.dataTransfer?.files) {
      void processFileList(e.dataTransfer.files);
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
      <div
        data-testid="har-dropzone"
        data-dragging={dragging ? 'true' : 'false'}
        className="hf-dropzone"
        onDragOver={handleDragOver}
        onDragLeave={handleDragLeave}
        onDrop={handleDrop}
      >
        <div style={{ fontWeight: 700 }}>{t.dashboard.dropzoneTitle}</div>
        <div style={{ fontSize: '0.82rem', color: 'var(--hf-text-muted)' }}>
          {t.dashboard.dropzoneHint}
        </div>
        <label className="hf-pill" style={{ cursor: 'pointer', marginTop: '0.35rem' }}>
          <span>{busy ? t.dashboard.importing : t.dashboard.dropzoneBrowse}</span>
          <input
            type="file"
            multiple
            accept=".har,.json,.gz"
            aria-label={t.dashboard.importHarLabel}
            onChange={handleChange}
            style={{ display: 'none' }}
          />
        </label>
      </div>

      {localError ? (
        <div className="hf-panel" role="alert" style={{ borderColor: '#ef4444' }}>
          {localError}
        </div>
      ) : null}
    </div>
  );
}
