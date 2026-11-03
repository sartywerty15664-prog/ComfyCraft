import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Blocks, Box, Gamepad2, Settings, Plus, Play, Download, Sparkles, Monitor, Trash2, Pencil, RefreshCw, CheckCircle2, Search } from "lucide-react";

type Page = "Play" | "Instances" | "Mods" | "Settings";
type Instance = { id: string; name: string; gameVersion: string; loader: string; createdAt: number };
type GameVersion = { id: string; versionType: string };
type JavaRuntime = { executable: string; version: string; major: number; vendor?: string; selected: boolean };
type InstallResult = { version: string; librariesDownloaded: number; assetsDownloaded: number; alreadyPresent: number };
type Readiness = { ready: boolean; checks: { id: string; label: string; ok: boolean; detail?: unknown; count?: number }[]; authentication: string };
type ModrinthMod = { projectId: string; slug: string; title: string; description: string; iconUrl?: string; downloads: number; author: string; latestVersion?: string };
type InstalledMod = { projectId: string; versionId: string; name: string; filename: string; sha1: string; active: boolean };
type ModUpdate = { projectId: string; currentVersionId: string; latestVersionId: string; latestName: string };
type ContentType = "mod" | "resourcepack" | "shader";
type LaunchInspection = { version: string; requiredJavaMajor: number; javaRuntime: { available: boolean; version?: string; major?: number }; jvmArgumentCount: number; gameArgumentCount: number; jvmArguments: string[]; gameArguments: string[]; unresolvedPlaceholders: string[]; authenticationRequired: boolean; launchEnabled: boolean; note: string };
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
  const [javaRuntimes, setJavaRuntimes] = useState<JavaRuntime[]>([]);
  const [javaLoading, setJavaLoading] = useState(false);
  const [versionsLoading, setVersionsLoading] = useState(true);
  const [versionsError, setVersionsError] = useState("");
  const [creating, setCreating] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [busy, setBusy] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [installingFabric, setInstallingFabric] = useState(false);
  const [installed, setInstalled] = useState(false);
  const [checkingLaunch, setCheckingLaunch] = useState(false);
  const [readiness, setReadiness] = useState<Readiness | null>(null);
  const [launchInspection, setLaunchInspection] = useState<LaunchInspection | null>(null);
  const [notice, setNotice] = useState("");
  const [error, setError] = useState("");
  const [modQuery, setModQuery] = useState("");
  const [modResults, setModResults] = useState<ModrinthMod[]>([]);
  const [installedMods, setInstalledMods] = useState<InstalledMod[]>([]);
  const [searchingMods, setSearchingMods] = useState(false);
  const [modBusy, setModBusy] = useState("");
  const [contentType, setContentType] = useState<ContentType>("mod");
  const [installedContent, setInstalledContent] = useState<InstalledMod[]>([]);
  const [modUpdates, setModUpdates] = useState<ModUpdate[]>([]);

  async function refreshJavaRuntimes() {
    setJavaLoading(true);
    try { setJavaRuntimes(await invoke<JavaRuntime[]>("list_java_runtimes")); }
    catch (e) { setError(String(e)); }
    finally { setJavaLoading(false); }
  }

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
    void refreshJavaRuntimes();
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

  useEffect(() => { void refreshInstallStatus(selectedId); void refreshInstalledMods(selectedId); void refreshContent(selectedId, contentType); void refreshModUpdates(selectedId); setModResults([]); }, [selectedId]);

  useEffect(() => { if (selectedId) void refreshContent(selectedId, contentType); setModResults([]); }, [contentType]);

  async function refreshInstalledMods(id: string) {
    if (!id) { setInstalledMods([]); return; }
    try { setInstalledMods(await invoke<InstalledMod[]>("list_installed_mods", { instanceId: id })); }
    catch (e) { setError(String(e)); }
  }

  async function refreshContent(id: string, type: ContentType) {
    if (!id) { setInstalledContent([]); return; }
    if (type === "mod") return;
    try { setInstalledContent(await invoke<InstalledMod[]>("list_installed_content", { instanceId: id, contentType: type })); }
    catch (e) { setError(String(e)); }
  }

  async function refreshModUpdates(id: string) {
    if (!id) { setModUpdates([]); return; }
    try { setModUpdates(await invoke<ModUpdate[]>("check_mod_updates", { instanceId: id })); }
    catch { setModUpdates([]); }
  }

  async function searchMods() {
    if (!selected) return;
    if (contentType === "mod" && !selected.loader.startsWith("Fabric ")) { setError("Install Fabric for this instance first."); return; }
    setSearchingMods(true); setError(""); setNotice("");
    try {
      const results = contentType === "mod"
        ? await invoke<ModrinthMod[]>("search_modrinth_mods", { instanceId: selected.id, query: modQuery })
        : await invoke<ModrinthMod[]>("search_modrinth_content", { instanceId: selected.id, query: modQuery, contentType });
      setModResults(results);
    } catch (e) { setError(String(e)); }
    finally { setSearchingMods(false); }
  }

  async function installMod(projectId: string) {
    if (!selected || modBusy) return;
    setModBusy(projectId); setError(""); setNotice("");
    try {
      if (contentType === "mod") { setInstalledMods(await invoke<InstalledMod[]>("install_modrinth_mod", { instanceId: selected.id, projectId })); await refreshModUpdates(selected.id); setNotice("Mod and required dependencies installed and verified."); }
      else { setInstalledContent(await invoke<InstalledMod[]>("install_modrinth_content", { instanceId: selected.id, projectId, contentType })); setNotice(contentType === "shader" ? "Shader pack installed and verified." : "Resource pack installed and verified."); }
    } catch (e) { setError(String(e)); }
    finally { setModBusy(""); }
  }

  async function toggleContent(projectId: string, enabled: boolean) {
    if (!selected || modBusy) return;
    setModBusy(projectId); setError(""); setNotice("");
    try {
      setInstalledContent(await invoke<InstalledMod[]>("set_modrinth_content_enabled", { instanceId: selected.id, projectId, contentType, enabled }));
      setNotice(enabled ? "Content enabled. Minecraft may need a resource reload." : "Content disabled.");
    } catch (e) { setError(String(e)); }
    finally { setModBusy(""); }
  }

  async function uninstallMod(projectId: string) {
    if (!selected || modBusy) return;
    setModBusy(projectId); setError(""); setNotice("");
    try {
      if (contentType === "mod") { setInstalledMods(await invoke<InstalledMod[]>("uninstall_modrinth_mod", { instanceId: selected.id, projectId })); await refreshModUpdates(selected.id); }
      else setInstalledContent(await invoke<InstalledMod[]>("uninstall_modrinth_content", { instanceId: selected.id, projectId, contentType }));
      setNotice("Managed content removed.");
    } catch (e) { setError(String(e)); }
    finally { setModBusy(""); }
  }

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

  async function installFabric() {
    if (!selected || installingFabric || !installed) return;
    setInstallingFabric(true); setError(""); setNotice("");
    try {
      const result = await invoke<{ loader: string; librariesDownloaded: number; librariesAlreadyPresent: number }>("install_fabric", { instanceId: selected.id });
      await refreshInstances();
      setNotice(`${result.loader} installed. Libraries: ${result.librariesDownloaded} downloaded; ${result.librariesAlreadyPresent} already present.`);
    } catch (e) { setError(String(e)); }
    finally { setInstallingFabric(false); }
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
            {installed && <button className="outline-button readiness-button" disabled={installingFabric || installing} onClick={() => void installFabric()}><Blocks size={15}/>{installingFabric ? "INSTALLING FABRIC…" : selected?.loader.startsWith("Fabric ") ? "REPAIR / UPDATE FABRIC" : "INSTALL FABRIC"}<span>Latest stable compatible loader</span></button>}
            {installed && <button className="outline-button readiness-button" disabled={checkingLaunch} onClick={async () => {
              if (!selected) return;
              setCheckingLaunch(true); setError(""); setNotice("");
              try {
                const result = await invoke<Readiness>("check_launch_readiness", { instanceId: selected.id });
                setReadiness(result);
                const inspection = await invoke<LaunchInspection>("inspect_launch_arguments", { instanceId: selected.id });
                setLaunchInspection(inspection);
                setNotice(result.ready ? "Runtime checks passed. Ely.by authentication is still required before launching." : "Some runtime checks need attention.");
              } catch (e) { setError(String(e)); }
              finally { setCheckingLaunch(false); }
            }}><RefreshCw size={15}/>{checkingLaunch ? "CHECKING…" : "CHECK LAUNCH READINESS"}</button>}
            {readiness && <div className="readiness-list">{readiness.checks.map((check) => <div key={check.id}><span className={check.ok ? "status-dot" : "status-dot status-fail"}/>{check.label}<b>{check.ok ? "OK" : "MISSING"}</b></div>)}<small>{readiness.authentication}</small></div>}
            {launchInspection && <div className="readiness-list"><div><span className={launchInspection.javaRuntime.available && (launchInspection.javaRuntime.major ?? 0) >= launchInspection.requiredJavaMajor ? "status-dot" : "status-dot status-fail"}/>Java runtime<b>{launchInspection.javaRuntime.version ?? "Not found"} · required {launchInspection.requiredJavaMajor}+</b></div><div><span className="status-dot"/>JVM arguments<b>{launchInspection.jvmArgumentCount}</b></div><div><span className="status-dot"/>Game arguments<b>{launchInspection.gameArgumentCount}</b></div><div><span className={launchInspection.unresolvedPlaceholders.length ? "status-dot status-fail" : "status-dot"}/>Unresolved placeholders<b>{launchInspection.unresolvedPlaceholders.length}</b></div><small>{launchInspection.note}</small><details className="argument-preview"><summary>Resolved JVM arguments</summary><pre>{launchInspection.jvmArguments.join("\n") || "None"}</pre></details><details className="argument-preview"><summary>Resolved game arguments</summary><pre>{launchInspection.gameArguments.join("\n") || "None"}</pre></details></div>}
            <p className="hint">Files are downloaded from official Mojang endpoints and checked against SHA-1 hashes and declared sizes.</p>
          </div>
          <div className="quick-grid"><div className="quick-card"><Blocks/><div><b>Mod manager</b><span>Modrinth · Fabric dependencies</span></div></div><div className="quick-card"><Monitor/><div><b>Graphics</b><span>Iris, shaders and resource packs</span></div></div></div>
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
        {page === "Mods" && <div className="page">
          <div className="eyebrow">MODRINTH · CONTENT LIBRARY</div><h2>Mods & graphics</h2>
          <p>Install compatible Fabric mods, resource packs and shader packs into the selected instance.</p>
          <div className="content-tabs">
            {([["mod","Mods"],["resourcepack","Resource packs"],["shader","Shaders"]] as [ContentType,string][]).map(([type,label]) => <button key={type} className={contentType === type ? "content-tab active" : "content-tab"} onClick={() => setContentType(type)}>{label}</button>)}
          </div>
          <div className="mod-search"><input value={modQuery} onChange={(e) => setModQuery(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") void searchMods(); }} placeholder={contentType === "mod" ? "Search mods on Modrinth…" : contentType === "shader" ? "Search shader packs…" : "Search resource packs…"} aria-label="Search Modrinth content"/><button className="primary-button" onClick={() => void searchMods()} disabled={searchingMods || !selected || (contentType === "mod" && !selected.loader.startsWith("Fabric "))}><Search size={15}/>{searchingMods ? "Searching…" : "Search"}</button></div>
          {!selected ? <div className="empty-state"><Box size={28}/><h3>Select an instance</h3><p>Choose an instance before browsing content.</p></div> : contentType === "mod" && !selected.loader.startsWith("Fabric ") ? <div className="empty-state"><Blocks size={28}/><h3>Fabric required</h3><p>Install Minecraft and Fabric for {selected.gameVersion} in the Play tab to browse mods.</p></div> : null}
          {modResults.length > 0 && <div className="mod-list">{modResults.map((item) => {
            const managed = contentType === "mod" ? installedMods.some((entry) => entry.projectId === item.projectId) : installedContent.some((entry) => entry.projectId === item.projectId);
            return <article className="mod-row" key={item.projectId}>
              {item.iconUrl ? <img className="mod-icon" src={item.iconUrl} alt="" /> : <div className="mod-icon fallback"><Blocks size={20}/></div>}
              <div className="mod-info"><b>{item.title}</b><span>by {item.author} · {item.downloads.toLocaleString()} downloads</span><p>{item.description}</p></div>
              <button className={managed ? "outline-button" : "primary-button"} disabled={Boolean(modBusy)} onClick={() => void (managed ? uninstallMod(item.projectId) : installMod(item.projectId))}>{modBusy === item.projectId ? "Working…" : managed ? "Uninstall" : "Install"}</button>
            </article>;
          })}</div>}
          {modResults.length === 0 && !searchingMods && selected && (contentType !== "mod" || selected.loader.startsWith("Fabric ")) && <div className="empty-state compact-empty"><Search size={28}/><h3>Find {contentType === "mod" ? "a mod" : contentType === "shader" ? "a shader pack" : "a resource pack"}</h3><p>Results are filtered for Minecraft {selected.gameVersion}{contentType === "mod" ? " and Fabric" : ""}.</p></div>}
          {contentType === "mod" ? <div className="installed-mods"><div className="section-kicker">INSTALLED MODS · {modUpdates.length} UPDATE{modUpdates.length === 1 ? "" : "S"} AVAILABLE</div>{installedMods.length === 0 ? <p className="muted">No managed mods installed yet.</p> : installedMods.map((item) => {
            const update = modUpdates.find((entry) => entry.projectId === item.projectId);
            return <div className="installed-mod-row" key={item.projectId}><span>{item.name}</span><small>{item.filename}</small>{update && <button className="text-action" disabled={Boolean(modBusy)} onClick={() => void installMod(item.projectId)}>{modBusy === item.projectId ? "Updating…" : "Update"}</button>}<button className="text-action" disabled={Boolean(modBusy)} onClick={() => void uninstallMod(item.projectId)}>Remove</button></div>;
          })}</div> : <div className="installed-mods"><div className="section-kicker">INSTALLED {contentType === "shader" ? "SHADERS" : "RESOURCE PACKS"}</div>{installedContent.length === 0 ? <p className="muted">No managed content installed yet.</p> : installedContent.map((item) => <div className="installed-mod-row" key={item.projectId}><span>{item.name}</span><small>{item.active ? "Enabled" : "Disabled"} · {item.filename}</small><button className="text-action" disabled={Boolean(modBusy)} onClick={() => void toggleContent(item.projectId, !item.active)}>{modBusy === item.projectId ? "Saving…" : item.active ? "Disable" : "Enable"}</button><button className="text-action" disabled={Boolean(modBusy)} onClick={() => void uninstallMod(item.projectId)}>Remove</button></div>)}</div>}
        </div>}
        {page === "Settings" && <div className="page"><div className="eyebrow">PREFERENCES</div><h2>Settings</h2><p>ComfyCraft is being designed for Linux desktops.</p><div className="settings-row"><div><b>Target platform</b><span>CachyOS / Arch Linux · Wayland / X11</span></div><strong>Linux only</strong></div><div className="settings-row"><div><b>Graphics testing</b><span>NVIDIA · KDE Plasma · Hyprland</span></div><strong>Planned</strong></div><div className="settings-row"><div><b>Account provider</b><span>Ely.by supported authentication flow</span></div><strong>Planned</strong></div><div className="settings-row"><div><b>Java Runtime</b><span>Detected runtimes · Java 25 recommended for Minecraft 26.3</span></div><button className="text-action" onClick={() => void refreshJavaRuntimes()} disabled={javaLoading}><RefreshCw size={14}/>{javaLoading ? "Scanning…" : "Rescan"}</button></div>{javaRuntimes.length === 0 ? <div className="empty-state"><Monitor size={25}/><h3>No Java runtime detected</h3><p>Install a Linux Java runtime, then rescan. ComfyCraft will not download runtimes from unverified sources.</p></div> : <div className="instance-list">{javaRuntimes.map((runtime) => <article className={runtime.selected ? "instance-row selected" : "instance-row"} key={runtime.executable}><div className="game-icon"><Monitor size={20}/></div><div className="instance-row-info"><b>Java {runtime.major} · {runtime.version}</b><span>{runtime.vendor ?? "Java runtime"} · {runtime.executable}</span></div><strong>{runtime.selected ? "Selected" : runtime.major >= 25 ? "Compatible with 26.3" : "Older runtime"}</strong>{!runtime.selected && <button className="text-action" onClick={async () => { try { setJavaRuntimes(await invoke<JavaRuntime[]>("select_java_runtime", { executable: runtime.executable })); setNotice("Java runtime selection saved."); } catch (e) { setError(String(e)); } }}>Use this</button>}</article>)}</div>}</div>}
      </section>
    </main>
  );
}
export default App;
