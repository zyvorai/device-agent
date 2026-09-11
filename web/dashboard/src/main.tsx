import React from 'react';
import ReactDOM from 'react-dom/client';
import {
  Activity, Box, Cable, CircuitBoard, Cpu, Gauge, Network, Radio, RefreshCw,
  Settings2, Thermometer, Usb, Wifi, Workflow
} from 'lucide-react';
import {
  age, bitrate, bytes, duration, getCanCaptureStatus, getDoctor, getIntegrations, getInventory,
  getRecentCanFrames, getRecentEvents, getSensors, getStatus
} from './api';
import type {
  AgentEvent, AgentStatus, CanCaptureStatus, CanFrame, DoctorReport, IntegrationStatus, Inventory, SensorSample
} from './types';
import './styles.css';

const demoInventory: Inventory = {
  device: { serial: 'ZY-MW-0001', vendor: 'Minewing', model: 'ARM64 Edge Gateway', hostname: 'edge-gateway-01', machine_id: 'demo' },
  system: { arch: 'aarch64', kernel: '6.8.0-edge', os: 'Ubuntu 24.04 LTS', cpu_model: '8-core ARM64', cpu_cores: 8, memory_bytes: 8 * 1024 ** 3, storage_bytes: 64 * 1024 ** 3, uptime_seconds: 238401 },
  network: [
    { name: 'eth0', kind: 'ethernet', operstate: 'up', mac: '02:42:ac:11:00:02', mtu: 1500, addresses: ['192.168.10.24/24'], rx_bytes: 123456789, tx_bytes: 38765432 },
    { name: 'wlan0', kind: 'wifi', operstate: 'down', mac: '02:42:ac:11:00:03', mtu: 1500, addresses: [] },
    { name: 'can0', kind: 'can', operstate: 'up', mtu: 16, addresses: [], rx_bytes: 1048576, tx_bytes: 983040, rx_errors: 0, tx_errors: 0 }
  ],
  buses: { gpio_chips: ['gpiochip0', 'gpiochip1'], i2c: ['i2c-0', 'i2c-1'], spi: ['spidev0.0'], uart: ['ttyS0', 'ttyS1', 'ttyUSB0'], can: ['can0', 'can1'], watchdog: ['watchdog0'] },
  industrial: {
    can: [
      { name: 'can0', kind: 'physical', operstate: 'up', driver: 'm_can_platform', mtu: 16, bitrate: 500000, data_bitrate: null, can_state: 'ERROR-ACTIVE', restart_ms: 100, tx_error_counter: 0, rx_error_counter: 0, rx_bytes: 1048576, tx_bytes: 983040, rx_errors: 0, tx_errors: 0, rx_dropped: 0, tx_dropped: 0, controller_modes: [], details_source: 'iproute2+sysfs' },
      { name: 'can1', kind: 'physical', operstate: 'down', driver: 'm_can_platform', mtu: 16, bitrate: 250000, data_bitrate: null, can_state: 'STOPPED', restart_ms: 0, tx_error_counter: 0, rx_error_counter: 0, rx_bytes: 0, tx_bytes: 0, rx_errors: 0, tx_errors: 0, rx_dropped: 0, tx_dropped: 0, controller_modes: [], details_source: 'iproute2+sysfs' }
    ],
    serial: [
      { name: 'ttyS0', path: '/dev/ttyS0', driver: '8250', transport: 'soc', rs485: null },
      { name: 'ttyS1', path: '/dev/ttyS1', driver: '8250', transport: 'soc', rs485: { declared: true, source: 'config+device-tree', enabled_at_boot: true, rts_active_high: true, rx_during_tx: false, delay_before_send_ms: 0, delay_after_send_ms: 0 } },
      { name: 'ttyUSB0', path: '/dev/ttyUSB0', driver: 'ch341', transport: 'usb', rs485: null }
    ]
  },
  usb: [{ path: '1-1', vendor_id: '1a86', product_id: '7523', manufacturer: 'QinHeng', product: 'USB Serial' }],
  thermal: [{ name: 'thermal_zone0', kind: 'soc', celsius: 47.2 }],
  capabilities: ['hardware-inventory', 'system-health', 'gpio', 'i2c', 'spi', 'uart', 'can', 'watchdog', 'sensor-plugin-api']
};

const demoIntegrations: IntegrationStatus = { nodra_enabled: true, nodra_connected: true, fleet_enabled: true, fleet_projection_ready: true };
const demoSensors: SensorSample[] = [{
  sensor_id: 'cabinet-temperature', plugin: 'i2c-temperature', collected_at_unix_ms: Date.now() - 1800,
  ok: true, quality: 'good', publish_to_nodra: true, readings: [{ name: 'cabinet-temperature', kind: 'temperature', value: 31.5, unit: 'celsius' }],
  labels: { bus: '/dev/i2c-1', address: '0x48', sensor_family: 'lm75' }, raw: {}
}];
const demoDoctor: DoctorReport = { ok: true, checks: [
  { name: 'machine-id', ok: true, detail: 'demo' }, { name: 'network-up', ok: true, detail: 'eth0, can0' },
  { name: 'profile.i2c', ok: true, detail: 'minimum 1, found 2' }, { name: 'profile.can', ok: true, detail: 'minimum 1, found 2' }
] };
const demoStatus: AgentStatus = { version: '0.1.3', inventory_generation: 12, last_inventory_refresh_unix_ms: Date.now() - 900, sensor_samples_total: 4210, sensor_sample_failures: 2, event_subscribers: 1, can_capture_frames_total: 18420, can_capture_dropped_total: 0 };
const demoCapture: CanCaptureStatus = { enabled: true, interfaces: ['can0'], frames_total: 18420, dropped_total: 0, decode_errors_total: 0, history_len: 6, subscribers: 1, last_error: null };
const demoFrames: CanFrame[] = [
  { sequence: 18420, interface: 'can0', captured_at_unix_ms: Date.now() - 150, can_id: 0x18ff50e5, extended: true, remote: false, error: false, fd: false, bitrate_switch: false, error_state_indicator: false, dlc: 8, data: [0,82,0,0,0,0,0,0], data_hex: '0052000000000000' },
  { sequence: 18419, interface: 'can0', captured_at_unix_ms: Date.now() - 420, can_id: 0x0cf00400, extended: true, remote: false, error: false, fd: false, bitrate_switch: false, error_state_indicator: false, dlc: 8, data: [255,125,80,0,0,0,0,0], data_hex: 'FF7D500000000000' }
];


type Page = 'Overview' | 'Hardware' | 'Interfaces' | 'Industrial' | 'Sensors' | 'Integrations' | 'Diagnostics' | 'Settings';
const pages: Array<[Page, React.ReactNode]> = [
  ['Overview', <Gauge size={18} />], ['Hardware', <Cpu size={18} />], ['Interfaces', <Cable size={18} />],
  ['Industrial', <CircuitBoard size={18} />], ['Sensors', <Thermometer size={18} />], ['Integrations', <Workflow size={18} />], ['Diagnostics', <Activity size={18} />], ['Settings', <Settings2 size={18} />]
];

function Dot({ on }: { on: boolean }) { return <span className={`dot ${on ? 'dot-on' : 'dot-off'}`} />; }
function Metric({ label, value, hint }: { label: string; value: string; hint?: string }) {
  return <div className="metric"><span>{label}</span><strong>{value}</strong>{hint && <small>{hint}</small>}</div>;
}
function StatusPill({ ok, children }: { ok: boolean; children: React.ReactNode }) {
  return <span className={`pill ${ok ? 'ok' : 'idle'}`}><Dot on={ok}/>{children}</span>;
}
function BusCard({ title, icon, values }: { title: string; icon: React.ReactNode; values: string[] }) {
  return <div className="card bus-card"><div className="card-title">{icon}<span>{title}</span><b>{values.length}</b></div><div className="chips">{values.length ? values.map(v => <span key={v}>{v}</span>) : <em>Not detected</em>}</div></div>;
}
function ReadingValue({ sample }: { sample: SensorSample }) {
  const reading = sample.readings[0];
  if (!sample.ok) return <strong className="sensor-value sensor-error">Error</strong>;
  if (!reading) return <strong className="sensor-value">Sampled</strong>;
  const rounded = Number.isInteger(reading.value) ? reading.value.toString() : reading.value.toFixed(2).replace(/0+$/, '').replace(/\.$/, '');
  const unit = reading.unit === 'celsius' ? '°C' : reading.unit === 'percent' ? '%' : reading.unit;
  return <strong className="sensor-value">{rounded}{unit ? ` ${unit}` : ''}</strong>;
}

function App() {
  const [page, setPage] = React.useState<Page>('Overview');
  const [inventory, setInventory] = React.useState<Inventory>(demoInventory);
  const [integrations, setIntegrations] = React.useState<IntegrationStatus>(demoIntegrations);
  const [sensors, setSensors] = React.useState<SensorSample[]>(demoSensors);
  const [doctor, setDoctor] = React.useState<DoctorReport>(demoDoctor);
  const [status, setStatus] = React.useState<AgentStatus>(demoStatus);
  const [events, setEvents] = React.useState<AgentEvent[]>([]);
  const [capture, setCapture] = React.useState<CanCaptureStatus>(demoCapture);
  const [canFrames, setCanFrames] = React.useState<CanFrame[]>(demoFrames);
  const [live, setLive] = React.useState(false);
  const [streamLive, setStreamLive] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);

  const refresh = React.useCallback(async () => {
    setRefreshing(true);
    try {
      const [inv, ints, samples, checks, agent, recent, captureStatus, frames] = await Promise.all([
        getInventory(), getIntegrations(), getSensors(), getDoctor(), getStatus(), getRecentEvents(), getCanCaptureStatus(), getRecentCanFrames()
      ]);
      setInventory(inv); setIntegrations(ints); setSensors(samples); setDoctor(checks); setStatus(agent); setEvents(recent.slice(-40)); setCapture(captureStatus); setCanFrames(frames.slice(-32)); setLive(true);
    } catch { setLive(false); }
    finally { setRefreshing(false); }
  }, []);

  React.useEffect(() => {
    refresh();
    const timer = window.setInterval(refresh, 15000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  React.useEffect(() => {
    const source = new EventSource('/api/v1/events');
    source.onopen = () => setStreamLive(true);
    source.onerror = () => setStreamLive(false);
    source.onmessage = (message) => {
      try {
        const event = JSON.parse(message.data) as AgentEvent;
        setEvents(current => [...current.slice(-39), event]);
        if (event.kind === 'inventory.changed' || event.kind === 'sensor.sample' || event.kind.startsWith('nodra.')) refresh();
      } catch { /* ignore malformed event */ }
    };
    return () => source.close();
  }, [refresh]);

  React.useEffect(() => {
    if (!capture.enabled) return;
    const source = new EventSource('/api/v1/can/frames/stream');
    source.addEventListener('can.frame', (message) => {
      try {
        const frame = JSON.parse((message as MessageEvent).data) as CanFrame;
        setCanFrames(current => [...current.slice(-31), frame]);
      } catch { /* ignore malformed frame */ }
    });
    return () => source.close();
  }, [capture.enabled]);

  const temp = inventory.thermal.map(z => z.celsius).filter((v): v is number => v != null).sort((a, b) => b - a)[0];
  const busCount = inventory.buses.gpio_chips.length + inventory.buses.i2c.length + inventory.buses.spi.length + inventory.buses.uart.length + inventory.buses.can.length;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark">Z</div><div><strong>Device Agent</strong><span>Zyvor Edge</span></div></div>
      <nav>{pages.map(([name, icon]) => <button key={name} className={page === name ? 'active' : ''} onClick={() => setPage(name)}>{icon}<span>{name}</span></button>)}</nav>
      <div className="side-foot"><StatusPill ok={live && streamLive}>{live ? (streamLive ? 'Live device' : 'API live') : 'Preview mode'}</StatusPill><span>v{status.version} · Apache-2.0</span></div>
    </aside>

    <main>
      <header><div><span className="eyebrow">{inventory.device.vendor} · {inventory.system.arch}</span><h1>{page}</h1></div><div className="header-actions"><span className="serial">{inventory.device.serial}</span><button className="refresh" onClick={refresh} aria-label="Refresh"><RefreshCw size={17} className={refreshing ? 'spin' : ''}/></button></div></header>

      {page === 'Overview' && <>
        <section className="hero card"><div><span className="eyebrow">EDGE NODE · GENERATION {status.inventory_generation}</span><h2>{inventory.device.model}</h2><p>{inventory.device.hostname} continuously watches Linux hardware, samples local sensors and exposes a stable edge contract.</p><div className="hero-pills"><StatusPill ok={integrations.nodra_connected}>Nodra</StatusPill><StatusPill ok={integrations.fleet_projection_ready}>Fleet</StatusPill><StatusPill ok={streamLive}>Events</StatusPill><StatusPill ok={doctor.ok}>Doctor</StatusPill></div></div><div className="orb"><Cpu size={44}/><span>{inventory.system.cpu_cores} core</span></div></section>
        <section className="metrics-grid"><Metric label="CPU" value={`${inventory.system.cpu_cores} cores`} hint={inventory.system.cpu_model}/><Metric label="Memory" value={bytes(inventory.system.memory_bytes)} hint={inventory.system.os}/><Metric label="Temperature" value={temp == null ? '—' : `${temp.toFixed(1)}°C`} hint={inventory.thermal[0]?.kind || 'No thermal zone'}/><Metric label="Sensors" value={`${sensors.filter(s => s.ok).length}/${sensors.length || 0}`} hint={`${status.sensor_samples_total} samples`}/></section>
        <section className="two-col"><div className="card"><div className="section-head"><div><span className="eyebrow">PHYSICAL I/O</span><h3>{busCount} detected interfaces</h3></div><Cable/></div><div className="io-line"><span>GPIO</span><b>{inventory.buses.gpio_chips.length}</b></div><div className="io-line"><span>I²C</span><b>{inventory.buses.i2c.length}</b></div><div className="io-line"><span>SPI</span><b>{inventory.buses.spi.length}</b></div><div className="io-line"><span>UART</span><b>{inventory.buses.uart.length}</b></div><div className="io-line"><span>CAN</span><b>{inventory.buses.can.length}</b></div></div>
          <div className="card dark-card"><span className="eyebrow">LIVE DATA PATH</span><h3>Hardware → Agent → Nodra → Fleet</h3><div className="flow"><span>Sensor</span><i>→</i><span>Agent</span><i>→</i><span>Nodra</span><i>→</i><span>Fleet</span></div><p>Inventory and status are retained in Nodra; sensor samples publish on per-sensor topics. Fleet consumes the hardware projection without duplicating its agent.</p></div></section>
      </>}

      {page === 'Hardware' && <section className="content-grid"><div className="card wide"><span className="eyebrow">COMPUTE</span><h2>{inventory.system.cpu_model}</h2><div className="spec-grid"><Metric label="Architecture" value={inventory.system.arch}/><Metric label="Memory" value={bytes(inventory.system.memory_bytes)}/><Metric label="Storage" value={bytes(inventory.system.storage_bytes)}/><Metric label="Uptime" value={duration(inventory.system.uptime_seconds)}/></div></div><div className="card"><span className="eyebrow">IDENTITY</span><h3>{inventory.device.serial}</h3><p className="muted">{inventory.device.vendor} · {inventory.device.model}</p><div className="mono">{inventory.device.machine_id}</div></div><div className="card"><span className="eyebrow">THERMAL</span><h3>{temp == null ? 'No sensor' : `${temp.toFixed(1)}°C`}</h3><p className="muted">Highest detected Linux thermal zone. Kernel {inventory.system.kernel}.</p></div></section>}

      {page === 'Interfaces' && <section className="content-grid"><BusCard title="GPIO" icon={<Radio size={18}/>} values={inventory.buses.gpio_chips}/><BusCard title="I²C" icon={<Cable size={18}/>} values={inventory.buses.i2c}/><BusCard title="SPI" icon={<Cable size={18}/>} values={inventory.buses.spi}/><BusCard title="UART / Serial" icon={<Cable size={18}/>} values={inventory.buses.uart}/><BusCard title="CAN" icon={<Network size={18}/>} values={inventory.buses.can}/><BusCard title="USB" icon={<Usb size={18}/>} values={inventory.usb.map(u => `${u.product || 'USB device'} · ${u.path}`)}/><div className="card wide"><div className="section-head"><div><span className="eyebrow">NETWORK</span><h3>Interfaces, addresses and counters</h3></div><Wifi/></div>{inventory.network.map(n => <div className="network-row network-rich" key={n.name}><div><Dot on={n.operstate === 'up'}/><b>{n.name}</b><span>{n.kind}</span><span>{n.addresses?.join(' · ') || 'no address'}</span></div><div><span>RX {bytes(n.rx_bytes)}</span><span>TX {bytes(n.tx_bytes)}</span><b>{n.operstate}</b></div></div>)}</div></section>}

      {page === 'Industrial' && <section className="industrial-layout">
        <div className="card wide industrial-hero"><div><span className="eyebrow">INDUSTRIAL I/O</span><h2>CAN health. RS485 awareness.</h2><p className="muted">Device Agent exposes physical bus state without interpreting machine protocols. J1939, Modbus RTU registers and device semantics remain in Nodra.</p></div><div className="industrial-stats"><Metric label="CAN" value={`${inventory.industrial.can.length}`} hint={`${inventory.industrial.can.filter(item => item.operstate === 'up').length} up`}/><Metric label="RS485" value={`${inventory.industrial.serial.filter(item => item.rs485).length}`} hint="declared ports"/><Metric label="Serial" value={`${inventory.industrial.serial.length}`} hint="UART / USB serial"/></div></div>
        <div className="industrial-grid wide">{inventory.industrial.can.length ? inventory.industrial.can.map(can => { const state = can.can_state || can.operstate; const busOff = state.toLowerCase() === 'bus-off'; return <article className="card industrial-card" key={can.name}><div className="section-head"><div><span className="eyebrow">SOCKETCAN · {can.kind.toUpperCase()}</span><h3>{can.name}</h3></div><StatusPill ok={!busOff && can.operstate === 'up'}>{state}</StatusPill></div><div className="industrial-rate">{bitrate(can.bitrate)}</div><div className="io-line"><span>Data bitrate</span><b>{bitrate(can.data_bitrate)}</b></div><div className="io-line"><span>Driver</span><b>{can.driver || 'unknown'}</b></div><div className="io-line"><span>Controller errors</span><b>RX {can.rx_error_counter ?? 0} · TX {can.tx_error_counter ?? 0}</b></div><div className="io-line"><span>Netdevice errors</span><b>RX {can.rx_errors ?? 0} · TX {can.tx_errors ?? 0}</b></div><div className="io-line"><span>Traffic</span><b>RX {bytes(can.rx_bytes)} · TX {bytes(can.tx_bytes)}</b></div><div className="chips">{can.controller_modes.length ? can.controller_modes.map(mode => <span key={mode}>{mode}</span>) : <em>{can.details_source}</em>}</div></article> }) : <article className="card industrial-card"><span className="eyebrow">SOCKETCAN</span><h3>No CAN interface detected</h3><p className="muted">Bring up a kernel CAN netdevice and it will appear here automatically.</p></article>}</div>
        <div className="card wide"><div className="section-head"><div><span className="eyebrow">READ-ONLY CAN CAPTURE</span><h3>{capture.enabled ? `${capture.interfaces.join(', ') || 'Configured'} · live frames` : 'Disabled by default'}</h3></div><StatusPill ok={capture.enabled && !capture.last_error}>{capture.enabled ? 'RX only' : 'off'}</StatusPill></div><p className="muted">Capture never transmits, never changes bitrate, and only binds explicitly allowlisted SocketCAN interfaces. Raw 29-bit identifiers are preserved for Nodra/J1939 processing.</p><div className="spec-grid"><Metric label="Frames" value={capture.frames_total.toLocaleString()}/><Metric label="Rate drops" value={capture.dropped_total.toLocaleString()}/><Metric label="Decode errors" value={capture.decode_errors_total.toLocaleString()}/><Metric label="History" value={`${capture.history_len}`}/></div>{capture.last_error && <p className="sensor-error-text">{capture.last_error}</p>}<div className="can-frame-list">{canFrames.slice(-8).reverse().map(frame => <div className="can-frame-row" key={`${frame.interface}-${frame.sequence}`}><span>{frame.interface}</span><code>{frame.extended ? frame.can_id.toString(16).padStart(8, '0').toUpperCase() : frame.can_id.toString(16).padStart(3, '0').toUpperCase()}</code><b>{frame.data_hex || '—'}</b><small>{frame.fd ? 'CAN-FD' : 'CAN'} · {age(frame.captured_at_unix_ms)}</small></div>)}</div></div>
        <div className="card wide"><div className="section-head"><div><span className="eyebrow">SERIAL / RS485</span><h3>Passive port inventory</h3></div><Cable/></div><p className="muted">RS485 is shown only when explicitly declared in configuration or described by the Linux device tree. The agent never transmits probe bytes.</p>{inventory.industrial.serial.map(port => <div className="serial-row" key={port.name}><div><StatusPill ok={Boolean(port.rs485)}>{port.rs485 ? 'RS485' : 'serial'}</StatusPill><div><b>{port.name}</b><span>{port.path} · {port.transport} · {port.driver || 'driver unknown'}</span></div></div><div className="serial-detail">{port.rs485 ? <><b>{port.rs485.source}</b><span>{port.rs485.enabled_at_boot ? 'enabled at boot' : 'runtime/config declared'}</span></> : <span>no RS485 declaration</span>}</div></div>)}</div>
      </section>}

      {page === 'Sensors' && <section className="sensor-grid">{sensors.length ? sensors.map(sample => <article className="card sensor-card" key={sample.sensor_id}><div className="section-head"><div><span className="eyebrow">{sample.plugin}</span><h3>{sample.sensor_id}</h3></div><StatusPill ok={sample.ok}>{sample.ok ? sample.quality : 'failed'}</StatusPill></div><ReadingValue sample={sample}/><div className="sensor-meta"><span>{age(sample.collected_at_unix_ms)}</span><span>{Object.entries(sample.labels).map(([k,v]) => `${k}=${v}`).join(' · ') || 'no labels'}</span></div>{sample.error && <p className="sensor-error-text">{sample.error}</p>}{sample.readings.slice(1).map(reading => <div className="io-line" key={reading.name}><span>{reading.name}</span><b>{reading.value} {reading.unit}</b></div>)}</article>) : <section className="empty-state card wide"><Thermometer size={32}/><span className="eyebrow">SENSOR SCHEDULER</span><h2>No samples yet.</h2><p>Install a plugin manifest under the configured plugins directory. The included LM75/TMP102 reference plugin reads an explicit I²C bus/address without scanning the bus.</p></section>}</section>}

      {page === 'Integrations' && <section className="integration-grid"><div className="card integration"><div className="integration-icon"><Radio/></div><span className="eyebrow">DATA PLANE</span><h2>Nodra</h2><p>Retained inventory/status, legacy telemetry and new per-sensor MQTT topics. Nodra continues to own WAL, protocol semantics and disconnected delivery.</p><StatusPill ok={integrations.nodra_connected}>{integrations.nodra_connected ? 'Connected' : integrations.nodra_enabled ? 'Configured · offline' : 'Disabled'}</StatusPill></div><div className="card integration"><div className="integration-icon"><Box/></div><span className="eyebrow">CONTROL PLANE</span><h2>Fleet</h2><p>Device Agent projects IP addresses, hardware counts, capabilities and identity into the existing Fleet inventory contract.</p><StatusPill ok={integrations.fleet_projection_ready}>{integrations.fleet_projection_ready ? 'Inventory bridge ready' : 'Disabled'}</StatusPill></div></section>}

      {page === 'Diagnostics' && <section className="diagnostic-layout"><div className="terminal card"><div className="terminal-head"><div className="lights"><i/><i/><i/></div><span>zyvor-device-agent doctor</span></div><pre>{doctor.checks.map(check => `${check.ok ? '✓' : '✕'} ${check.name.padEnd(22)} ${check.detail}`).join('\n') || 'No diagnostic checks returned.'}</pre></div><div className="card event-card"><div className="section-head"><div><span className="eyebrow">EVENT STREAM</span><h3>{streamLive ? 'Streaming' : 'Disconnected'}</h3></div><Activity/></div><div className="event-list">{events.slice().reverse().map(event => <div className="event-row" key={`${event.id}-${event.at_unix_ms}`}><Dot on={!event.kind.includes('disconnected')}/><div><b>{event.kind}</b><span>{age(event.at_unix_ms)}</span></div></div>)}</div></div></section>}

      {page === 'Settings' && <section className="content-grid"><div className="card wide"><span className="eyebrow">RUNTIME</span><h2>Minewing reference ARM64</h2><p className="muted">The agent refreshes hardware continuously, samples plugins independently from browser traffic, and exports CAN/RS485 health without decoding industrial protocols. Modbus/J1939 semantics stay in Nodra.</p><div className="settings-line"><span>REST API</span><code>:9188</code></div><div className="settings-line"><span>Inventory generation</span><code>{status.inventory_generation}</code></div><div className="settings-line"><span>Last refresh</span><code>{age(status.last_inventory_refresh_unix_ms)}</code></div><div className="settings-line"><span>Sensor failures</span><code>{status.sensor_sample_failures}</code></div><div className="settings-line"><span>Event subscribers</span><code>{status.event_subscribers}</code></div></div></section>}
    </main>
  </div>;
}

ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
