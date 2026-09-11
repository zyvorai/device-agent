export type BusInventory = {
  gpio_chips: string[];
  i2c: string[];
  spi: string[];
  uart: string[];
  can: string[];
  watchdog?: string[];
};

export type Inventory = {
  device: { serial: string; vendor: string; model: string; hostname: string; machine_id: string };
  system: { arch: string; kernel: string; os: string; cpu_model: string; cpu_cores: number; memory_bytes: number; storage_bytes: number | null; uptime_seconds: number };
  network: Array<{ name: string; kind: string; operstate: string; mac?: string | null; mtu?: number | null }>;
  buses: BusInventory;
  usb: Array<{ path: string; vendor_id?: string | null; product_id?: string | null; manufacturer?: string | null; product?: string | null }>;
  thermal: Array<{ name: string; kind: string; celsius?: number | null }>;
  capabilities: string[];
};

export type IntegrationStatus = {
  nodra_enabled: boolean;
  nodra_connected: boolean;
  fleet_enabled: boolean;
  fleet_projection_ready: boolean;
};
