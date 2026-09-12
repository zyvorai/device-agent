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

/** Appends the stored bearer token as a `?token=` query param, for EventSource
 *  connections (browsers cannot set custom headers on EventSource). */
export function withTokenParam(path: string): string {
  const token = getToken();
  if (!token) return path;
  const separator = path.includes('?') ? '&' : '?';
  return `${path}${separator}token=${encodeURIComponent(token)}`;
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

/** Not a JSON fetch - these back an <img src>, which can't set an
 *  Authorization header, so the token (when set) travels as `?token=`
 *  via withTokenParam, same as the CAN/events SSE streams. */
export function cameraSnapshotUrl(id: string): string {
  return withTokenParam(`/api/v1/camera/${encodeURIComponent(id)}/snapshot`);
}

export function cameraStreamUrl(id: string): string {
  return withTokenParam(`/api/v1/camera/${encodeURIComponent(id)}/stream`);
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
