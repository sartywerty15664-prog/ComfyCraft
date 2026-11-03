import { useState } from "react";
import { Blocks, Box, Gamepad2, Settings, Plus, Play, Download, Sparkles, Monitor, ChevronDown } from "lucide-react";

type Page = "Play" | "Instances" | "Mods" | "Settings";

const nav: { label: Page; icon: typeof Gamepad2 }[] = [
  { label: "Play", icon: Gamepad2 },
  { label: "Instances", icon: Box },
  { label: "Mods", icon: Blocks },
  { label: "Settings", icon: Settings },
];

function App() {
  const [page, setPage] = useState<Page>("Play");
  const [instance, setInstance] = useState("Performance 26.3");

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand"><div className="brand-mark">C</div><div><strong>ComfyCraft</strong><span>JAVA EDITION</span></div></div>
        <div className="nav-label">WORKSPACE</div>
        <nav>{nav.map(({ label, icon: Icon }) => <button key={label} className={page === label ? "nav-item active" : "nav-item"} onClick={() => setPage(label)}><Icon size={18}/>{label}</button>)}</nav>
        <div className="sidebar-bottom"><div className="linux-pill"><span/> LINUX BUILD</div><small>Early development · v0.1.0</small></div>
      </aside>

      <section className="content">
        <header className="topbar"><div className="breadcrumb">ComfyCraft <span>/</span> {page}</div><div className="top-status"><span className="status-dot"/> Launcher shell ready</div></header>
        {page === "Play" && <div className="page play-page">
          <div className="eyebrow"><Sparkles size={14}/> YOUR MINECRAFT SPACE</div>
          <h1>Ready when<br/>you are.</h1>
          <p className="lead">Your worlds, mods and settings — in one comfortable place.</p>
          <div className="launch-card">
            <div className="card-top"><div className="game-icon"><Box size={25}/></div><div className="instance-info"><span>SELECTED INSTANCE</span><strong>{instance}</strong><small>Minecraft Java Edition · 26.3 target</small></div><button className="select-button"><ChevronDown size={16}/></button></div>
            <div className="card-divider"/>
            <div className="launch-meta"><div><span>LOADER</span><b>Not installed</b></div><div><span>JAVA</span><b>Java 25 target</b></div><div><span>PROFILE</span><b>Performance</b></div></div>
            <button className="play-button" disabled><Play size={17} fill="currentColor"/> PLAY <span>Coming soon</span></button>
            <p className="hint">The game installer and launch pipeline are the next development milestone.</p>
          </div>
          <div className="quick-grid"><div className="quick-card"><Blocks/><div><b>Mod manager</b><span>Modrinth integration planned</span></div></div><div className="quick-card"><Monitor/><div><b>Graphics</b><span>Iris, shaders and resource packs</span></div></div></div>
        </div>}
        {page === "Instances" && <div className="page"><div className="page-heading"><div><div className="eyebrow">YOUR SETUPS</div><h2>Instances</h2><p>Keep separate modded and vanilla installations.</p></div><button className="outline-button" disabled><Plus size={16}/> New instance</button></div><div className="empty-state"><Box size={30}/><h3>No instances yet</h3><p>Instance creation will arrive with the game file manager.</p></div></div>}
        {page === "Mods" && <div className="page"><div className="eyebrow">MODRINTH</div><h2>Mods</h2><p>Discover mods and let ComfyCraft resolve their dependencies.</p><div className="empty-state"><Download size={30}/><h3>Mod browser is being built</h3><p>Search, compatibility checks, dependency installation and updates are planned.</p></div></div>}
        {page === "Settings" && <div className="page"><div className="eyebrow">PREFERENCES</div><h2>Settings</h2><p>ComfyCraft is being designed for Linux desktops.</p><div className="settings-row"><div><b>Target platform</b><span>CachyOS / Arch Linux · Wayland / X11</span></div><strong>Linux only</strong></div><div className="settings-row"><div><b>Graphics testing</b><span>NVIDIA · KDE Plasma · Hyprland</span></div><strong>Planned</strong></div><div className="settings-row"><div><b>Account provider</b><span>Ely.by supported authentication flow</span></div><strong>Planned</strong></div></div>}
      </section>
    </main>
  );
}

export default App;
