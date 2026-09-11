import type { IntegrationStatus, Inventory } from './types';

export async function getInventory(): Promise<Inventory> {
  const response = await fetch('/api/v1/inventory');
  if (!response.ok) throw new Error(`inventory: ${response.status}`);
  return response.json();
}

export async function getIntegrations(): Promise<IntegrationStatus> {
  const response = await fetch('/api/v1/integrations');
  if (!response.ok) throw new Error(`integrations: ${response.status}`);
  return response.json();
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
