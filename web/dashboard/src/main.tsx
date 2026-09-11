import React from 'react';
import ReactDOM from 'react-dom/client';
import {
  Activity, Box, Cable, CheckCircle2, Cpu, Gauge, HardDrive, Network,
  Radio, RefreshCw, Settings2, Thermometer, Usb, Wifi, Workflow
} from 'lucide-react';
import { bytes, duration, getIntegrations, getInventory } from './api';
import type { IntegrationStatus, Inventory } from './types';
import './styles.css';

const demoInventory: Inventory = {
  device: { serial: 'ZY-MW-0001', vendor: 'Minewing', model: 'ARM64 Edge Gateway', hostname: 'edge-gateway-01', machine_id: 'demo' },
  system: { arch: 'aarch64', kernel: '6.8.0-edge', os: 'Ubuntu 24.04 LTS', cpu_model: '8-core ARM64', cpu_cores: 8, memory_bytes: 8 * 1024 ** 3, storage_bytes: 64 * 1024 ** 3, uptime_seconds: 238401 },
  network: [
    { name: 'eth0', kind: 'ethernet', operstate: 'up', mac: '02:42:ac:11:00:02', mtu: 1500 },
    { name: 'wlan0', kind: 'wifi', operstate: 'down', mac: '02:42:ac:11:00:03', mtu: 1500 },
    { name: 'can0', kind: 'can', operstate: 'up', mtu: 16 }
  ],
  buses: { gpio_chips: ['gpiochip0', 'gpiochip1'], i2c: ['i2c-0', 'i2c-1'], spi: ['spidev0.0'], uart: ['ttyS0', 'ttyS1', 'ttyUSB0'], can: ['can0', 'can1'], watchdog: ['watchdog0'] },
  usb: [{ path: '1-1', vendor_id: '1a86', product_id: '7523', manufacturer: 'QinHeng', product: 'USB Serial' }],
  thermal: [{ name: 'thermal_zone0', kind: 'soc', celsius: 47.2 }],
  capabilities: ['hardware-inventory', 'system-health', 'gpio', 'i2c', 'spi', 'uart', 'can', 'watchdog', 'sensor-plugin-api']
};

const demoIntegrations: IntegrationStatus = { nodra_enabled: true, nodra_connected: true, fleet_enabled: true, fleet_projection_ready: true };

type Page = 'Overview' | 'Hardware' | 'Interfaces' | 'Sensors' | 'Integrations' | 'Diagnostics' | 'Settings';
const pages: Array<[Page, React.ReactNode]> = [
  ['Overview', <Gauge size={18} />], ['Hardware', <Cpu size={18} />], ['Interfaces', <Cable size={18} />],
  ['Sensors', <Thermometer size={18} />], ['Integrations', <Workflow size={18} />], ['Diagnostics', <Activity size={18} />], ['Settings', <Settings2 size={18} />]
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

function App() {
  const [page, setPage] = React.useState<Page>('Overview');
  const [inventory, setInventory] = React.useState<Inventory>(demoInventory);
  const [integrations, setIntegrations] = React.useState<IntegrationStatus>(demoIntegrations);
  const [live, setLive] = React.useState(false);
  const [refreshing, setRefreshing] = React.useState(false);

  const refresh = React.useCallback(async () => {
    setRefreshing(true);
    try {
      const [inv, ints] = await Promise.all([getInventory(), getIntegrations()]);
      setInventory(inv); setIntegrations(ints); setLive(true);
    } catch { setLive(false); }
    finally { setRefreshing(false); }
  }, []);

  React.useEffect(() => { refresh(); const timer = window.setInterval(refresh, 10000); return () => window.clearInterval(timer); }, [refresh]);

  const temp = inventory.thermal.find(z => z.celsius != null)?.celsius;
  const busCount = inventory.buses.gpio_chips.length + inventory.buses.i2c.length + inventory.buses.spi.length + inventory.buses.uart.length + inventory.buses.can.length;

  return <div className="app-shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark">Z</div><div><strong>Device Agent</strong><span>Zyvor Edge</span></div></div>
      <nav>{pages.map(([name, icon]) => <button key={name} className={page === name ? 'active' : ''} onClick={() => setPage(name)}>{icon}<span>{name}</span></button>)}</nav>
      <div className="side-foot"><StatusPill ok={live}>{live ? 'Live device' : 'Preview mode'}</StatusPill><span>v0.1.0 · Apache-2.0</span></div>
    </aside>

    <main>
      <header><div><span className="eyebrow">{inventory.device.vendor} · {inventory.system.arch}</span><h1>{page}</h1></div><div className="header-actions"><span className="serial">{inventory.device.serial}</span><button className="refresh" onClick={refresh} aria-label="Refresh"><RefreshCw size={17} className={refreshing ? 'spin' : ''}/></button></div></header>

      {page === 'Overview' && <>
        <section className="hero card"><div><span className="eyebrow">EDGE NODE</span><h2>{inventory.device.model}</h2><p>{inventory.device.hostname} is healthy and ready for local hardware workloads.</p><div className="hero-pills"><StatusPill ok={integrations.nodra_connected}>Nodra</StatusPill><StatusPill ok={integrations.fleet_projection_ready}>Fleet</StatusPill><StatusPill ok={inventory.network.some(n => n.operstate === 'up')}>Network</StatusPill></div></div><div className="orb"><Cpu size={44}/><span>{inventory.system.cpu_cores} core</span></div></section>
        <section className="metrics-grid"><Metric label="CPU" value={`${inventory.system.cpu_cores} cores`} hint={inventory.system.cpu_model}/><Metric label="Memory" value={bytes(inventory.system.memory_bytes)} hint={inventory.system.os}/><Metric label="Temperature" value={temp == null ? '—' : `${temp.toFixed(1)}°C`} hint={inventory.thermal[0]?.kind || 'No thermal zone'}/><Metric label="Uptime" value={duration(inventory.system.uptime_seconds)} hint={inventory.system.kernel}/></section>
        <section className="two-col"><div className="card"><div className="section-head"><div><span className="eyebrow">PHYSICAL I/O</span><h3>{busCount} detected interfaces</h3></div><Cable/></div><div className="io-line"><span>GPIO</span><b>{inventory.buses.gpio_chips.length}</b></div><div className="io-line"><span>I²C</span><b>{inventory.buses.i2c.length}</b></div><div className="io-line"><span>SPI</span><b>{inventory.buses.spi.length}</b></div><div className="io-line"><span>UART</span><b>{inventory.buses.uart.length}</b></div><div className="io-line"><span>CAN</span><b>{inventory.buses.can.length}</b></div></div>
          <div className="card dark-card"><span className="eyebrow">DATA PATH</span><h3>Hardware → Nodra → Fleet</h3><div className="flow"><span>Sensor</span><i>→</i><span>Agent</span><i>→</i><span>Nodra</span><i>→</i><span>Fleet</span></div><p>The agent discovers and exposes hardware. Nodra owns protocol meaning, local routing and offline delivery.</p></div></section>
      </>}

      {page === 'Hardware' && <section className="content-grid"><div className="card wide"><span className="eyebrow">COMPUTE</span><h2>{inventory.system.cpu_model}</h2><div className="spec-grid"><Metric label="Architecture" value={inventory.system.arch}/><Metric label="Memory" value={bytes(inventory.system.memory_bytes)}/><Metric label="Storage" value={bytes(inventory.system.storage_bytes)}/><Metric label="Kernel" value={inventory.system.kernel}/></div></div><div className="card"><span className="eyebrow">IDENTITY</span><h3>{inventory.device.serial}</h3><p className="muted">{inventory.device.vendor} · {inventory.device.model}</p><div className="mono">{inventory.device.machine_id}</div></div><div className="card"><span className="eyebrow">THERMAL</span><h3>{temp == null ? 'No sensor' : `${temp.toFixed(1)}°C`}</h3><p className="muted">Read from Linux thermal zones.</p></div></section>}

      {page === 'Interfaces' && <section className="content-grid"><BusCard title="GPIO" icon={<Radio size={18}/>} values={inventory.buses.gpio_chips}/><BusCard title="I²C" icon={<Cable size={18}/>} values={inventory.buses.i2c}/><BusCard title="SPI" icon={<Cable size={18}/>} values={inventory.buses.spi}/><BusCard title="UART / Serial" icon={<Cable size={18}/>} values={inventory.buses.uart}/><BusCard title="CAN" icon={<Network size={18}/>} values={inventory.buses.can}/><BusCard title="USB" icon={<Usb size={18}/>} values={inventory.usb.map(u => `${u.product || 'USB device'} · ${u.path}`)}/><div className="card wide"><div className="section-head"><div><span className="eyebrow">NETWORK</span><h3>Linux interfaces</h3></div><Wifi/></div>{inventory.network.map(n => <div className="network-row" key={n.name}><div><Dot on={n.operstate === 'up'}/><b>{n.name}</b><span>{n.kind}</span></div><div><span>{n.mac || '—'}</span><b>{n.operstate}</b></div></div>)}</div></section>}

      {page === 'Sensors' && <section className="empty-state card"><Thermometer size={32}/><span className="eyebrow">PLUGIN API</span><h2>Drivers stay small. Protocol meaning stays in Nodra.</h2><p>Sensor plugins emit typed JSON samples over the Device Agent plugin contract. v0.1 includes the manifest format and sample command runner; I²C/serial reference plugins are the next implementation step.</p><button>Open plugin contract</button></section>}

      {page === 'Integrations' && <section className="integration-grid"><div className="card integration"><div className="integration-icon"><Radio/></div><span className="eyebrow">DATA PLANE</span><h2>Nodra</h2><p>MQTT publishing, protocol adapters, local processing and disconnected WAL.</p><StatusPill ok={integrations.nodra_connected}>{integrations.nodra_connected ? 'Connected' : integrations.nodra_enabled ? 'Configured · offline' : 'Disabled'}</StatusPill></div><div className="card integration"><div className="integration-icon"><Box/></div><span className="eyebrow">CONTROL PLANE</span><h2>Fleet</h2><p>Registration, desired state, rollout, application lifecycle and health.</p><StatusPill ok={integrations.fleet_projection_ready}>{integrations.fleet_projection_ready ? 'Inventory bridge ready' : 'Disabled'}</StatusPill></div></section>}

      {page === 'Diagnostics' && <section className="terminal card"><div className="terminal-head"><div className="lights"><i/><i/><i/></div><span>zyvor-device-agent doctor</span></div><pre>{`$ zyvor-device-agent doctor\n\n✓ machine-id        ${inventory.device.machine_id.slice(0, 18)}…\n✓ network           ${inventory.network.length} interfaces detected\n✓ buses             ${busCount} physical interfaces\n✓ thermal           ${inventory.thermal.length} thermal zone(s)\n${integrations.nodra_connected ? '✓' : '○'} nodra             ${integrations.nodra_connected ? 'connected' : 'not connected'}\n${integrations.fleet_projection_ready ? '✓' : '○'} fleet bridge      ${integrations.fleet_projection_ready ? 'inventory projection ready' : 'disabled'}\n\n${live ? 'All local checks passed.' : 'Dashboard is showing preview data; API is not reachable.'}`}</pre></section>}

      {page === 'Settings' && <section className="content-grid"><div className="card wide"><span className="eyebrow">DEVICE PROFILE</span><h2>Minewing reference ARM64</h2><p className="muted">Profiles describe expected physical interfaces and board-specific aliases without hard-coding protocol semantics into the daemon.</p><div className="settings-line"><span>REST API</span><code>:9188</code></div><div className="settings-line"><span>Inventory refresh</span><code>10s</code></div><div className="settings-line"><span>Runtime</span><code>systemd · Linux</code></div></div></section>}
    </main>
  </div>;
}

ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><App /></React.StrictMode>);
