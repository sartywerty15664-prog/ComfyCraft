import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Blocks, Box, Gamepad2, Settings, Plus, Play, Download, Sparkles, Monitor, Trash2, Pencil, RefreshCw, CheckCircle2 } from "lucide-react";

type Page = "Play" | "Instances" | "Mods" | "Settings";
type Instance = { id: string; name: string; gameVersion: string; loader: string; createdAt: number };
type GameVersion = { id: string; versionType: string };
type InstallResult = { version: string; librariesDownloaded: number; assetsDownloaded: number; alreadyPresent: number };
const nav: { label: Page; icon: typeof Gamepad2 }[] = [
  { label: "Play", icon: Gamepad2 }, { label: "Instances", icon: Box },
  { label: "Mods", icon: Blocks }, { label: "Settings", icon: Settings },
];

function App() {
  const [page, setPage] = useState<Page>("Play");
  const [instances, setInstances] = useState<Instance[]>([]);
  const [selectedId, setSelectedId] = useState("");
  const [newName, setNewName] = useState("");
  const [newVersion, setNewVersion] = useState("");
  const [gameVersions, setGameVersions] = useState<GameVersion[]>([]);
  const [versionsLoading, setVersionsLoading] = useState(true);
  const [versionsError, setVersionsError] = useState("");
  const [creating, setCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [busy, setBusy] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installed, setInstalled] = useState(false);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");

  async function refreshInstances() {
    try {
      const data = await invoke<Instance[]>("list_instances");
      setInstances(data);
      setSelectedId((current) => data.some((item) => item.id === current) ? current : (data[0]?.id ?? ""));
      setError("");
    } catch (e) { setError(String(e)); }
  }

  async function refreshInstallStatus(id: string) {
    if (!id) { setInstalled(false); return; }
    try { setInstalled(await invoke<boolean>("is_game_installed", { instanceId: id })); }
    catch { setInstalled(false); }
  }

  useEffect(() => {
    void refreshInstances();
    void (async () => {
      try {
        const versions = await invoke<GameVersion[]>("list_game_versions");
        setGameVersions(versions);
        setNewVersion((current) => current && versions.some((v) => v.id === current) ? current : (versions[0]?.id ?? ""));
        setVersionsError(versions.length ? "" : "No official release versions were returned.");
      } catch (e) { setVersionsError(String(e)); }
      finally { setVersionsLoading(false); }
    })();
  }, []);

  useEffect(() => { void refreshInstallStatus(selectedId); }, [selectedId]);

  const selected = instances.find((item) => item.id === selectedId);

  async function createInstance() {
    if (!newName.trim() || !newVersion) return;
    setBusy(true); setError(""); setNotice("");
    try {
      const created = await invoke<Instance>("create_instance", { name: newName, gameVersion: newVersion });
      await refreshInstances(); setSelectedId(created.id); setNewName(""); setCreating(false);
      setNotice("Instance created. Install its Minecraft files to continue.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function installSelectedGame() {
    if (!selected || installing) return;
    setInstalling(true); setError(""); setNotice("");
    try {
      const result = await invoke<InstallResult>("install_game", { instanceId: selected.id });
      setInstalled(true);
      setNotice(`Minecraft ${result.version} installed and verified. Libraries: ${result.librariesDownloaded} downloaded; assets: ${result.assetsDownloaded} downloaded.`);
    } catch (e) { setError(String(e)); }
    finally { setInstalling(false); }
  }

  async function saveRename(id: string) {
    setBusy(true); setError("");
    try {
      await invoke<Instance>("rename_instance", { id, name: editName });
      await refreshInstances(); setEditingId(null); setNotice("Instance renamed.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function removeInstance(item: Instance) {
    if (!window.confirm(`Delete "${item.name}" and all files inside this instance?`)) return;
    setBusy(true); setError("");
    try { await invoke("delete_instance", { id: item.id }); await refreshInstances(); setNotice("Instance deleted."); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand"><div className="brand-mark">C</div><div><strong>ComfyCraft</strong><span>JAVA EDITION</span></div></div>
        <div className="nav-label">WORKSPACE</div>
        <nav>{nav.map(({ label, icon: Icon }) => <button key={label} className={page === label ? "nav-item active" : "nav-item"} onClick={() => setPage(label)}><Icon size={18}/>{label}</button>)}</nav>
        <div className="sidebar-bottom"><div className="linux-pill"><span/> LINUX BUILD</div><small>Early development · v0.1.0</small></div>
      </aside>
      <section className="content">
        <header className="topbar"><div className="breadcrumb">ComfyCraft <span>/</span> {page}</div><div className="top-status"><span className="status-dot"/> Instance storage ready</div></header>
        {(error || notice) && <div className={error ? "notice error" : "notice"}>{error || notice}<button onClick={() => { setError(""); setNotice(""); }}>×</button></div>}
        {page === "Play" && <div className="page play-page">
          <div className="eyebrow"><Sparkles size={14}/> YOUR MINECRAFT SPACE</div>
          <h1>Ready when<br/>you are.</h1>
          <p className="lead">Your worlds, mods and settings — in one comfortable place.</p>
          <div className="launch-card">
            <div className="card-top"><div className="game-icon"><Box size={25}/></div><div className="instance-info"><span>SELECTED INSTANCE</span><strong>{selected?.name ?? "No instance selected"}</strong><small>{selected ? `Minecraft Java Edition · ${selected.gameVersion} · ${selected.loader}` : "Create an instance to get started"}</small></div>
              <select aria-label="Select instance" className="instance-select" value={selectedId} onChange={(e) => setSelectedId(e.target.value)}><option value="">Select</option>{instances.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select>
            </div>
            <div className="card-divider"/>
            <div className="launch-meta"><div><span>LOADER</span><b>{selected?.loader ?? "—"}</b></div><div><span>JAVA</span><b>Java 25 target</b></div><div><span>PROFILE</span><b>Performance</b></div></div>
            <button className="play-button" onClick={() => void installSelectedGame()} disabled={!selected || installing || busy}>
              {installed ? <CheckCircle2 size={17}/> : <Download size={17}/>}
              {installing ? "INSTALLING…" : installed ? "VERIFY / REPAIR GAME FILES" : "INSTALL MINECRAFT"}
              <span>{installing ? "Downloading official files" : installed ? "Files installed" : "Client · libraries · assets"}</span>
            </button>
            <p className="hint">Files are downloaded from official Mojang endpoints and checked against SHA-1 hashes and declared sizes.</p>
          </div>
          <div className="quick-grid"><div className="quick-card"><Blocks/><div><b>Mod manager</b><span>Modrinth integration planned</span></div></div><div className="quick-card"><Monitor/><div><b>Graphics</b><span>Iris, shaders and resource packs</span></div></div></div>
        </div>}
        {page === "Instances" && <div className="page">
          <div className="page-heading"><div><div className="eyebrow">YOUR SETUPS</div><h2>Instances</h2><p>Separate vanilla and modded installations. Each instance has its own mods, worlds and settings.</p></div><button className="outline-button" onClick={() => setCreating((v) => !v)}><Plus size={16}/> New instance</button></div>
          {creating && <form className="create-panel" onSubmit={(e) => { e.preventDefault(); void createInstance(); }}>
            <label>INSTANCE NAME<input autoFocus maxLength={40} value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="My survival setup"/></label>
            <label>MINECRAFT VERSION<select value={newVersion} onChange={(e) => setNewVersion(e.target.value)} disabled={versionsLoading || gameVersions.length === 0}>
              {versionsLoading && <option value="">Loading official versions…</option>}
              {gameVersions.map((version) => <option key={version.id} value={version.id}>{version.id}</option>)}
            </select></label>
            <button className="primary-button" disabled={busy || !newName.trim() || !newVersion}><Plus size={16}/> Create instance</button>
            {versionsError && <p className="version-error">{versionsError}</p>}
          </form>}
          {instances.length === 0 ? <div className="empty-state"><Box size={30}/><h3>No instances yet</h3><p>Create your first isolated Minecraft setup.</p></div> :
            <div className="instance-list">{instances.map((item) => <article className={selectedId === item.id ? "instance-row selected" : "instance-row"} key={item.id}>
              <div className="game-icon"><Box size={22}/></div><div className="instance-row-info">{editingId === item.id ? <form className="rename-form" onSubmit={(e) => { e.preventDefault(); void saveRename(item.id); }}><input autoFocus maxLength={40} value={editName} onChange={(e) => setEditName(e.target.value)}/><button className="text-action">Save</button><button type="button" className="text-action" onClick={() => setEditingId(null)}>Cancel</button></form> : <><b>{item.name}</b><span>Minecraft {item.gameVersion} · {item.loader}</span></>}</div>
              <button className="text-action" onClick={() => setSelectedId(item.id)}>{selectedId === item.id ? "Selected" : "Select"}</button>
              <button className="icon-action" aria-label="Rename instance" onClick={() => { setEditingId(item.id); setEditName(item.name); }}><Pencil size={15}/></button>
              <button className="icon-action danger" aria-label="Delete instance" onClick={() => void removeInstance(item)} disabled={busy}><Trash2 size={15}/></button>
            </article>)}</div>}
          <button className="refresh-button" onClick={() => void refreshInstances()}><RefreshCw size={14}/> Refresh list</button>
        </div>}
        {page === "Mods" && <div className="page"><div className="eyebrow">MODRINTH</div><h2>Mods</h2><p>Discover mods and let ComfyCraft resolve their dependencies.</p><div className="empty-state"><Download size={30}/><h3>Mod browser is next</h3><p>Search, compatibility checks, dependency installation and updates are planned.</p></div></div>}
        {page === "Settings" && <div className="page"><div className="eyebrow">PREFERENCES</div><h2>Settings</h2><p>ComfyCraft is being designed for Linux desktops.</p><div className="settings-row"><div><b>Target platform</b><span>CachyOS / Arch Linux · Wayland / X11</span></div><strong>Linux only</strong></div><div className="settings-row"><div><b>Graphics testing</b><span>NVIDIA · KDE Plasma · Hyprland</span></div><strong>Planned</strong></div><div className="settings-row"><div><b>Account provider</b><span>Ely.by supported authentication flow</span></div><strong>Planned</strong></div></div>}
      </section>
    </main>
  );
}
export default App;
