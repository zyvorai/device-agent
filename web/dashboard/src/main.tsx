import React from 'react';
import ReactDOM from 'react-dom/client';
import {
  Activity, Box, Cable, Camera, Cpu, Gauge, Network, Radio, RefreshCw, Thermometer, Usb, Wifi
} from 'lucide-react';
import {
  age, AuthRequiredError, bitrate, bytes, cameraSnapshotUrl, cameraStreamUrl, consumeSse, duration,
  getCameras, getCanCaptureStatus, getCommissioning, getDoctor, getFindings, getIntegrations, getInventory,
  getPassport, getRecentCanFrames, getRecentEvents, getRecorder, getSensors, getStatus, getToken,
  previewSupportBundle, setToken, withStreamTicket
} from './api';
import type {
  AgentEvent, AgentStatus, CameraCaptureStatus, CanCaptureStatus, CanFrame, DoctorReport,
  IntegrationStatus, Inventory, SensorSample
} from './types';
import './styles.css';

const demoInventory: Inventory = {
  device: { serial: 'ZY-REF-0001', vendor: 'Generic', model: 'ARM64 Edge Gateway', hostname: 'edge-gateway-01', machine_id: 'demo' },
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
const demoCameras: CameraCaptureStatus[] = [];


type Page = 'Overview' | 'Passport' | 'Timeline' | 'Interfaces' | 'Diagnostics' | 'Support';
const pages: Array<[Page, React.ReactNode]> = [
  ['Overview', <Gauge size={18} />], ['Passport', <Cpu size={18} />], ['Timeline', <Activity size={18} />],
  ['Interfaces', <Cable size={18} />], ['Diagnostics', <Activity size={18} />], ['Support', <Box size={18} />]
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
  const [cameras, setCameras] = React.useState<CameraCaptureStatus[]>(demoCameras);
  const [live, setLive] = React.useState(false);
  const [streamLive, setStreamLive] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);
  const [authRequired, setAuthRequired] = React.useState(false);
  const [tokenInput, setTokenInput] = React.useState(getToken());
  const [wizard, setWizard] = React.useState(false);
  const [passport, setPassport] = React.useState<Record<string, unknown> | null>(null);
  const [findings, setFindings] = React.useState<Array<Record<string, unknown>>>([]);
  const [timeline, setTimeline] = React.useState<Array<Record<string, unknown>>>([]);
  const [bundlePreview, setBundlePreview] = React.useState<string>('');
  const [cameraUrls, setCameraUrls] = React.useState<Record<string, string>>({});

  const refresh = React.useCallback(async () => {
    setRefreshing(true);
    try {
      const [inv, ints, samples, checks, agent, recent, captureStatus, frames, cameraStatuses, pass, found, recorded, commissioning] = await Promise.all([
        getInventory(), getIntegrations(), getSensors(), getDoctor(), getStatus(), getRecentEvents(), getCanCaptureStatus(), getRecentCanFrames(), getCameras(),
        getPassport().catch(() => null), getFindings().catch(() => []), getRecorder('15m').catch(() => []), getCommissioning().catch(() => null)
      ]);
      setInventory(inv); setIntegrations(ints); setSensors(samples); setDoctor(checks); setStatus(agent); setEvents(recent.slice(-40)); setCapture(captureStatus); setCanFrames(frames.slice(-32)); setCameras(cameraStatuses); setPassport(pass); setFindings(found); setTimeline(recorded); setLive(true); setAuthRequired(false);
      if (commissioning?.needsWizard && window.localStorage.getItem('zyvor-wizard-dismissed') !== '1') setWizard(true);
    } catch (error) {
      setLive(false);
      setAuthRequired(error instanceof AuthRequiredError);
    }
    finally { setRefreshing(false); }
  }, []);

  const saveToken = React.useCallback((value: string) => {
    setToken(value.trim());
    setTokenInput(value.trim());
    refresh();
  }, [refresh]);

  React.useEffect(() => {
    // Standard fetch-on-mount: refresh() sets a "refreshing" flag synchronously
    // before its first await, which is what the lint rule below flags - the
    // actual data-driven state updates all happen after the async API calls.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    refresh();
    const timer = window.setInterval(refresh, 15000);
    return () => window.clearInterval(timer);
  }, [refresh]);

  React.useEffect(() => {
    if (authRequired) return;
    const controller = new AbortController();
    consumeSse('/api/v1/events', (_event, data) => {
      try {
        const event = JSON.parse(data) as AgentEvent;
        setEvents(current => [...current.slice(-39), event]);
        setStreamLive(true);
        if (event.kind === 'inventory.changed' || event.kind === 'sensor.sample' || event.kind.startsWith('nodra.')) refresh();
      } catch { /* ignore malformed event */ }
    }, controller.signal).catch(() => setStreamLive(false));
    return () => controller.abort();
  }, [refresh, authRequired, tokenInput]);

  React.useEffect(() => {
    if (!capture.enabled || authRequired) return;
    const controller = new AbortController();
    consumeSse('/api/v1/can/frames/stream', (event, data) => {
      if (event !== 'can.frame') return;
      try {
        const frame = JSON.parse(data) as CanFrame;
        setCanFrames(current => [...current.slice(-31), frame]);
      } catch { /* ignore malformed frame */ }
    }, controller.signal).catch(() => undefined);
    return () => controller.abort();
  }, [capture.enabled, authRequired, tokenInput]);

  React.useEffect(() => {
    let cancel = false;
    Promise.all(cameras.map(async (camera) => [camera.id, await withStreamTicket(cameraStreamUrl(camera.id))] as const))
      .then((pairs) => { if (!cancel) setCameraUrls(Object.fromEntries(pairs)); })
      .catch(() => undefined);
    return () => { cancel = true; };
  }, [cameras, tokenInput]);

  const temp = inventory.thermal.map(z => z.celsius).filter((v): v is number => v != null).sort((a, b) => b - a)[0];
  const busCount = inventory.buses.gpio_chips.length + inventory.buses.i2c.length + inventory.buses.spi.length + inventory.buses.uart.length + inventory.buses.can.length;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark">Z</div><div><strong>Device Agent</strong><span>Zyvor Edge</span></div></div>
      <nav>{pages.map(([name, icon]) => <button key={name} className={page === name ? 'active' : ''} onClick={() => setPage(name)}>{icon}<span>{name}</span></button>)}</nav>
      <div className="side-foot"><StatusPill ok={live && streamLive}>{authRequired ? 'Authentication required' : live ? (streamLive ? 'Live device' : 'API live') : 'Preview mode'}</StatusPill><span>v{status.version} · Apache-2.0</span></div>
    </aside>

    <main>
      <header><div><span className="eyebrow">{inventory.device.vendor} · {inventory.system.arch}</span><h1>{page}</h1></div><div className="header-actions"><span className="serial">{inventory.device.serial}</span><button className="refresh" onClick={refresh} aria-label="Refresh"><RefreshCw size={17} className={refreshing ? 'spin' : ''}/></button></div></header>

      {authRequired && <section className="card auth-banner" role="alert">
        <div><span className="eyebrow">AUTHENTICATION REQUIRED</span><h3>This Device Agent requires a bearer token</h3><p className="muted">Enter the token issued when the agent was deployed (see <code>scripts/deploy-remote.sh</code>'s output, or <code>sudo cat /etc/zyvor/device-agent/auth/bearer.token</code>).</p></div>
        <form className="auth-banner-form" onSubmit={(event) => { event.preventDefault(); saveToken(tokenInput); }}>
          <input type="password" placeholder="Bearer token" value={tokenInput} onChange={(event) => setTokenInput(event.target.value)} aria-label="Bearer token" />
          <button type="submit">Connect</button>
        </form>
      </section>}

      {wizard && <section className="card auth-banner" role="dialog">
        <div><span className="eyebrow">FIRST RUN</span><h3>Commission this device without editing TOML first</h3><p className="muted">Review the Overview, then set a bearer token or enroll with Fleet. Minew silicon is not certified until a physical HIL run is signed.</p></div>
        <button type="button" onClick={() => { window.localStorage.setItem('zyvor-wizard-dismissed', '1'); setWizard(false); }}>Continue</button>
      </section>}

      {page === 'Overview' && <>
        <section className="hero card"><div><span className="eyebrow">TRUSTED HARDWARE PASSPORT</span><h2>{inventory.device.model}</h2><p>{inventory.device.hostname} reports health, identity, Fleet and Nodra status for remote diagnosis.</p><div className="hero-pills"><StatusPill ok={doctor.ok}>Healthy</StatusPill><StatusPill ok={integrations.nodra_connected}>Nodra</StatusPill><StatusPill ok={integrations.fleet_projection_ready}>Fleet</StatusPill><StatusPill ok={Boolean(passport?.signature)}>Passport signed</StatusPill></div></div><div className="orb"><Cpu size={44}/><span>{inventory.system.cpu_cores} core</span></div></section>
        <section className="metrics-grid"><Metric label="CPU" value={`${inventory.system.cpu_cores} cores`} hint={inventory.system.cpu_model}/><Metric label="Memory" value={bytes(inventory.system.memory_bytes)} hint={inventory.system.os}/><Metric label="Temperature" value={temp == null ? '—' : `${temp.toFixed(1)}°C`} hint={inventory.thermal[0]?.kind || 'No thermal zone'}/><Metric label="Interfaces" value={`${busCount}`} hint={`${sensors.filter(s => s.ok).length} sensors ok`}/></section>
      </>}

      {page === 'Passport' && <section className="content-grid"><div className="card wide"><span className="eyebrow">DEVICE PASSPORT</span><h2>{String(passport?.deviceId || inventory.device.serial)}</h2><p className="muted">Hardware identity, firmware, TPM policy and qualification for Fleet, Yard and support.</p><pre className="mono">{passport ? JSON.stringify(passport, null, 2) : 'Passport unavailable'}</pre></div></section>}

      {page === 'Timeline' && <section className="diagnostic-layout"><div className="card event-card wide"><div className="section-head"><div><span className="eyebrow">FLIGHT RECORDER</span><h3>Last 15 minutes</h3></div><Activity/></div><div className="event-list">{(timeline.length ? timeline : events).slice().reverse().map((event, index) => <div className="event-row" key={`${String(event.kind)}-${index}`}><Dot on={!String(event.kind).includes('disconnected')}/><div><b>{String(event.kind)}</b><span>{typeof event.at_unix_ms === 'number' ? age(event.at_unix_ms as number) : ''}</span></div></div>)}</div></div></section>}

      {page === 'Interfaces' && <section className="content-grid">
        <BusCard title="GPIO" icon={<Radio size={18}/>} values={inventory.buses.gpio_chips}/>
        <BusCard title="I²C" icon={<Cable size={18}/>} values={inventory.buses.i2c}/>
        <BusCard title="SPI" icon={<Cable size={18}/>} values={inventory.buses.spi}/>
        <BusCard title="UART / Serial" icon={<Cable size={18}/>} values={inventory.buses.uart}/>
        <BusCard title="CAN" icon={<Network size={18}/>} values={inventory.buses.can}/>
        <BusCard title="USB" icon={<Usb size={18}/>} values={inventory.usb.map(u => `${u.product || 'USB device'} · ${u.path}`)}/>
        <div className="card wide"><div className="section-head"><div><span className="eyebrow">NETWORK</span><h3>Interfaces</h3></div><Wifi/></div>{inventory.network.map(n => <div className="network-row network-rich" key={n.name}><div><Dot on={n.operstate === 'up'}/><b>{n.name}</b><span>{n.kind}</span><span>{n.addresses?.join(' · ') || 'no address'}</span></div><div><span>RX {bytes(n.rx_bytes)}</span><span>TX {bytes(n.tx_bytes)}</span><b>{n.operstate}</b></div></div>)}</div>
        <div className="card wide"><div className="section-head"><div><span className="eyebrow">CAN HEALTH</span><h3>{capture.enabled ? 'Capture running' : 'Capture off'}</h3></div><StatusPill ok={capture.enabled && !capture.last_error}>{capture.enabled ? 'RX only' : 'off'}</StatusPill></div>{inventory.industrial.can.map(can => <div className="io-line" key={can.name}><span>{can.name}</span><b>{can.can_state || can.operstate} · {bitrate(can.bitrate)}</b></div>)}<div className="can-frame-list">{canFrames.slice(-6).reverse().map(frame => <div className="can-frame-row" key={`${frame.interface}-${frame.sequence}`}><span>{frame.interface}</span><code>{frame.data_hex || '—'}</code><small>{age(frame.captured_at_unix_ms)}</small></div>)}</div></div>
        <div className="card wide"><div className="section-head"><div><span className="eyebrow">SENSORS</span><h3>{sensors.length} samples</h3></div><Thermometer/></div>{sensors.length ? sensors.map(sample => <div className="io-line" key={sample.sensor_id}><span>{sample.sensor_id}</span><b>{sample.ok ? sample.quality : 'failed'}</b></div>) : <p className="muted">No sensor plugins have reported yet.</p>}</div>
        <div className="card wide"><div className="section-head"><div><span className="eyebrow">CAMERAS</span><h3>{cameras.length} devices</h3></div><Camera/></div>{cameras.length ? cameras.map(camera => <article key={camera.id}><StatusPill ok={camera.capturing}>{camera.id}</StatusPill>{camera.capturing ? <img className="camera-stream" src={cameraUrls[camera.id] || cameraStreamUrl(camera.id)} alt={camera.id}/> : <a className="link-button" href={cameraSnapshotUrl(camera.id)} target="_blank" rel="noreferrer">Snapshot</a>}</article>) : <p className="muted">No cameras configured.</p>}</div>
      </section>}

      {page === 'Diagnostics' && <section className="diagnostic-layout"><div className="terminal card"><div className="terminal-head"><div className="lights"><i/><i/><i/></div><span>zyvor-device-agent doctor</span></div><pre>{doctor.checks.map(check => `${check.ok ? '✓' : '✕'} ${check.name.padEnd(22)} ${check.detail}`).join('\n') || 'No diagnostic checks returned.'}</pre></div><div className="card event-card"><div className="section-head"><div><span className="eyebrow">FINDINGS</span><h3>{findings.length || 'None'}</h3></div><Activity/></div>{findings.length ? findings.map((finding, index) => <div className="event-row" key={index}><div><b>{String(finding.observation)}</b><span>{String(finding.confidence)}</span><p className="muted">{Array.isArray(finding.likelyCauses) ? (finding.likelyCauses as string[]).join(', ') : ''}</p></div></div>) : <p className="muted">No rules matched. Edge AI inference remains not-configured.</p>}</div></section>}

      {page === 'Support' && <section className="content-grid"><div className="card wide"><span className="eyebrow">SUPPORT BUNDLE</span><h2>Redacted evidence for remote diagnosis</h2><p className="muted">Preview the manifest before export. Secrets, IPs, MACs and hostnames are redacted by default.</p><button type="button" onClick={() => previewSupportBundle().then((manifest) => setBundlePreview(JSON.stringify(manifest, null, 2))).catch((error) => setBundlePreview(String(error)))}>Preview manifest</button><pre className="mono">{bundlePreview || 'Click preview to inspect the signed manifest.'}</pre></div>
        <div className="card"><span className="eyebrow">AUTHENTICATION</span><h3>Bearer token</h3><form className="auth-banner-form" onSubmit={(event) => { event.preventDefault(); saveToken(tokenInput); }}><input type="password" placeholder="Bearer token" value={tokenInput} onChange={(event) => setTokenInput(event.target.value)} aria-label="Bearer token" /><button type="submit">Save</button></form>{getToken() && <button className="link-button" onClick={() => saveToken('')}>Clear stored token</button>}</div></section>}
    </main>
  </div>;
}

ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
