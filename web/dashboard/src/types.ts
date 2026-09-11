export type BusInventory = {
  gpio_chips: string[];
  i2c: string[];
  spi: string[];
  uart: string[];
  can: string[];
  watchdog?: string[];
};

export type NetworkInterface = {
  name: string;
  kind: string;
  operstate: string;
  mac?: string | null;
  mtu?: number | null;
  addresses?: string[];
  rx_bytes?: number | null;
  tx_bytes?: number | null;
  rx_errors?: number | null;
  tx_errors?: number | null;
};

export type Inventory = {
  device: { serial: string; vendor: string; model: string; hostname: string; machine_id: string };
  system: { arch: string; kernel: string; os: string; cpu_model: string; cpu_cores: number; memory_bytes: number; storage_bytes: number | null; uptime_seconds: number };
  network: NetworkInterface[];
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

export type SensorReading = {
  name: string;
  kind: string;
  value: number;
  unit: string;
};

export type SensorSample = {
  sensor_id: string;
  plugin: string;
  collected_at_unix_ms: number;
  ok: boolean;
  quality: string;
  publish_to_nodra: boolean;
  readings: SensorReading[];
  labels: Record<string, string>;
  error?: string | null;
  raw: unknown;
};

export type DoctorReport = {
  ok: boolean;
  checks: Array<{ name: string; ok: boolean; detail: string }>;
};

export type AgentEvent = {
  id: number;
  kind: string;
  at_unix_ms: number;
  data: unknown;
};

export type AgentStatus = {
  version: string;
  inventory_generation: number;
  last_inventory_refresh_unix_ms: number;
  sensor_samples_total: number;
  sensor_sample_failures: number;
  event_subscribers: number;
};
