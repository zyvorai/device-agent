import type { AgentEvent, AgentStatus, CameraCaptureStatus, CanCaptureStatus, CanFrame, DoctorReport, IntegrationStatus, Inventory, SensorSample } from './types';

const TOKEN_STORAGE_KEY = 'zyvor-device-agent.bearer-token';

export function getToken(): string {
  try { return window.localStorage.getItem(TOKEN_STORAGE_KEY) ?? ''; }
  catch { return ''; }
}

export function setToken(token: string): void {
  try {
    if (token) window.localStorage.setItem(TOKEN_STORAGE_KEY, token);
    else window.localStorage.removeItem(TOKEN_STORAGE_KEY);
  } catch { /* localStorage unavailable (private mode, etc.) — token just won't persist */ }
}

/** Camera and other header-less surfaces use a short-lived ticket, never the bearer. */
export async function withStreamTicket(path: string): Promise<string> {
  const token = getToken();
  if (!token) return path;
  const response = await fetch('/api/v1/stream-tickets', {
    method: 'POST',
    headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' },
    body: JSON.stringify({ path_prefix: path }),
  });
  if (!response.ok) return path;
  const body = await response.json() as { ticket?: string };
  if (!body.ticket) return path;
  const separator = path.includes('?') ? '&' : '?';
  return `${path}${separator}ticket=${encodeURIComponent(body.ticket)}`;
}

export async function consumeSse(
  path: string,
  onEvent: (event: string, data: string) => void,
  signal: AbortSignal,
): Promise<void> {
  const token = getToken();
  const headers: HeadersInit = token ? { Authorization: `Bearer ${token}` } : {};
  const response = await fetch(path, { headers, signal });
  if (response.status === 401) throw new AuthRequiredError(path);
  if (!response.ok || !response.body) throw new Error(`${path}: ${response.status}`);
  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  while (!signal.aborted) {
    const { value, done } = await reader.read();
    if (done) break;
    buffer += decoder.decode(value, { stream: true });
    const chunks = buffer.split('\n\n');
    buffer = chunks.pop() ?? '';
    for (const chunk of chunks) {
      let event = 'message';
      const dataLines: string[] = [];
      for (const line of chunk.split('\n')) {
        if (line.startsWith('event:')) event = line.slice(6).trim();
        else if (line.startsWith('data:')) dataLines.push(line.slice(5).trim());
      }
      if (dataLines.length) onEvent(event, dataLines.join('\n'));
    }
  }
}

/** Thrown specifically for a 401 response, so callers can distinguish
 *  "authentication required" from a plain network/connectivity failure. */
export class AuthRequiredError extends Error {
  constructor(path: string) {
    super(`${path}: authentication required`);
    this.name = 'AuthRequiredError';
  }
}

async function getJson<T>(path: string): Promise<T> {
  const token = getToken();
  const headers: HeadersInit = token ? { Authorization: `Bearer ${token}` } : {};
  const response = await fetch(path, { headers });
  if (response.status === 401) throw new AuthRequiredError(path);
  if (!response.ok) throw new Error(`${path}: ${response.status}`);
  return response.json();
}

export function getInventory(): Promise<Inventory> {
  return getJson('/api/v1/inventory');
}

export function getIntegrations(): Promise<IntegrationStatus> {
  return getJson('/api/v1/integrations');
}

export function getSensors(): Promise<SensorSample[]> {
  return getJson('/api/v1/sensors');
}

export function getDoctor(): Promise<DoctorReport> {
  return getJson('/api/v1/doctor');
}

export function getRecentEvents(): Promise<AgentEvent[]> {
  return getJson('/api/v1/events/recent');
}

export function getStatus(): Promise<AgentStatus> {
  return getJson('/api/v1/status');
}

export function getCanCaptureStatus(): Promise<CanCaptureStatus> {
  return getJson('/api/v1/can/capture');
}

export function getRecentCanFrames(): Promise<CanFrame[]> {
  return getJson('/api/v1/can/frames/recent');
}

export function getCameras(): Promise<CameraCaptureStatus[]> {
  return getJson('/api/v1/camera');
}

export function cameraSnapshotUrl(id: string): string {
  return `/api/v1/camera/${encodeURIComponent(id)}/snapshot`;
}

export function cameraStreamUrl(id: string): string {
  return `/api/v1/camera/${encodeURIComponent(id)}/stream`;
}

export function getPassport(): Promise<Record<string, unknown>> {
  return getJson('/api/v1/passport');
}

export function getFindings(): Promise<Array<Record<string, unknown>>> {
  return getJson('/api/v1/diagnostics/findings');
}

export function getRecorder(since = '15m'): Promise<Array<Record<string, unknown>>> {
  return getJson(`/api/v1/recorder?since=${encodeURIComponent(since)}`);
}

export function getCommissioning(): Promise<{ needsWizard: boolean; authMode: string; identityPresent: boolean }> {
  return getJson('/api/v1/commissioning');
}

export function previewSupportBundle(): Promise<Record<string, unknown>> {
  const token = getToken();
  return fetch('/api/v1/support-bundle/preview', {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      ...(token ? { Authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify({ since: '2h', redact: true }),
  }).then(async (response) => {
    if (response.status === 401) throw new AuthRequiredError('/api/v1/support-bundle/preview');
    if (!response.ok) throw new Error(`support preview: ${response.status}`);
    return response.json();
  });
}

export function bytes(value: number | null | undefined): string {
  if (!value) return '—';
  const units = ['B', 'KiB', 'MiB', 'GiB', 'TiB'];
  let n = value;
  let i = 0;
  while (n >= 1024 && i < units.length - 1) { n /= 1024; i += 1; }
  return `${n >= 10 || i === 0 ? n.toFixed(0) : n.toFixed(1)} ${units[i]}`;
}

export function duration(seconds: number): string {
  const days = Math.floor(seconds / 86400);
  const hours = Math.floor((seconds % 86400) / 3600);
  if (days) return `${days}d ${hours}h`;
  const minutes = Math.floor((seconds % 3600) / 60);
  return `${hours}h ${minutes}m`;
}

export function age(unixMs: number): string {
  const seconds = Math.max(0, Math.round((Date.now() - unixMs) / 1000));
  if (seconds < 60) return `${seconds}s ago`;
  if (seconds < 3600) return `${Math.floor(seconds / 60)}m ago`;
  return `${Math.floor(seconds / 3600)}h ago`;
}

export function bitrate(value: number | null | undefined): string {
  if (!value) return '—';
  if (value >= 1_000_000) return `${(value / 1_000_000).toFixed(value % 1_000_000 === 0 ? 0 : 1)} Mbit/s`;
  if (value >= 1_000) return `${(value / 1_000).toFixed(value % 1_000 === 0 ? 0 : 1)} kbit/s`;
  return `${value} bit/s`;
}
