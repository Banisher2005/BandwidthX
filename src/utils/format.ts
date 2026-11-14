export function formatBytes(bytes: number, decimals = 1): string {
  if (bytes <= 0 || isNaN(bytes)) return '0 B';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  if (i < 0) return '0 B';
  const val = parseFloat((bytes / Math.pow(k, i)).toFixed(dm));
  return `${val} ${sizes[i] || 'PB'}`;
}

export function formatSpeed(bytesPerSec: number, decimals = 1): string {
  if (bytesPerSec <= 0 || isNaN(bytesPerSec)) return '0 B/s';
  const k = 1024;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s'];
  const i = Math.floor(Math.log(bytesPerSec) / Math.log(k));
  if (i < 0) return '0 B/s';
  const val = parseFloat((bytesPerSec / Math.pow(k, i)).toFixed(dm));
  return `${val} ${sizes[i] || 'GB/s'}`;
}

export function formatBitsSpeed(bytesPerSec: number, decimals = 1): string {
  const bits = bytesPerSec * 8;
  if (bits <= 0 || isNaN(bits)) return '0 bps';
  const k = 1000;
  const dm = decimals < 0 ? 0 : decimals;
  const sizes = ['bps', 'Kbps', 'Mbps', 'Gbps'];
  const i = Math.floor(Math.log(bits) / Math.log(k));
  if (i < 0) return '0 bps';
  const val = parseFloat((bits / Math.pow(k, i)).toFixed(dm));
  return `${val} ${sizes[i] || 'Gbps'}`;
}

export function truncateMiddle(text: string, maxLength: number): string {
  if (!text || text.length <= maxLength) return text;
  const half = Math.floor((maxLength - 3) / 2);
  return `${text.slice(0, half)}...${text.slice(-half)}`;
}

export function getFileNameFromPath(path: string): string {
  if (!path) return '';
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}
