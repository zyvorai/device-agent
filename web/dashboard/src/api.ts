import type { AgentEvent, AgentStatus, DoctorReport, IntegrationStatus, Inventory, SensorSample } from './types';

async function getJson<T>(path: string): Promise<T> {
  const response = await fetch(path);
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
