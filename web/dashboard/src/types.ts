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

export type CanInterfaceInfo = {
  name: string;
  kind: string;
  operstate: string;
  driver?: string | null;
  mtu?: number | null;
  bitrate?: number | null;
  data_bitrate?: number | null;
  can_state?: string | null;
  restart_ms?: number | null;
  tx_error_counter?: number | null;
  rx_error_counter?: number | null;
  rx_bytes?: number | null;
  tx_bytes?: number | null;
  rx_errors?: number | null;
  tx_errors?: number | null;
  rx_dropped?: number | null;
  tx_dropped?: number | null;
  controller_modes: string[];
  details_source: string;
};

export type Rs485Info = {
  declared: boolean;
  source: string;
  enabled_at_boot?: boolean | null;
  rts_active_high?: boolean | null;
  rx_during_tx?: boolean | null;
  delay_before_send_ms?: number | null;
  delay_after_send_ms?: number | null;
};

export type SerialPortInfo = {
  name: string;
  path: string;
  driver?: string | null;
  transport: string;
  rs485?: Rs485Info | null;
};

export type IndustrialInventory = {
  can: CanInterfaceInfo[];
  serial: SerialPortInfo[];
};

export type CanFrame = {
  sequence: number;
  interface: string;
  captured_at_unix_ms: number;
  can_id: number;
  extended: boolean;
  remote: boolean;
  error: boolean;
  fd: boolean;
  bitrate_switch: boolean;
  error_state_indicator: boolean;
  dlc: number;
  data: number[];
  data_hex: string;
};

export type CanCaptureStatus = {
  enabled: boolean;
  interfaces: string[];
  frames_total: number;
  dropped_total: number;
  decode_errors_total: number;
  history_len: number;
  subscribers: number;
  last_error?: string | null;
};

export type CameraCaptureStatus = {
  id: string;
  enabled: boolean;
  capturing: boolean;
  frames_total: number;
  dropped_total: number;
  encode_errors_total: number;
  subscribers: number;
  last_error?: string | null;
  last_frame_at_unix_ms?: number | null;
};

export type Inventory = {
  device: { serial: string; vendor: string; model: string; hostname: string; machine_id: string };
  system: { arch: string; kernel: string; os: string; cpu_model: string; cpu_cores: number; memory_bytes: number; storage_bytes: number | null; uptime_seconds: number };
  network: NetworkInterface[];
  buses: BusInventory;
  industrial: IndustrialInventory;
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
  can_capture_frames_total: number;
  can_capture_dropped_total: number;
};
