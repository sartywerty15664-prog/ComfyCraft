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
type AssetRepairResult = { indexId: string; downloaded: number; alreadyValid: number; total: number; status: string };
type AssetVerification = { indexId: string; total: number; verified: number; missing: number; corrupt: number; bytesVerified: number; workersUsed: number; elapsedMs: number; problemSamples: string[] };
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
  const [javaInstalling, setJavaInstalling] = useState(false);
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
  const [verifyingAssets, setVerifyingAssets] = useState(false);
  const [repairingAssets, setRepairingAssets] = useState(false);
  const [assetVerification, setAssetVerification] = useState<AssetVerification | null>(null);
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
    catch (e) { setError(`Ошибка: ${String(e)}`); }
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
    if (contentType === "mod" && !selected.loader.startsWith("Fabric ")) { setError("Сначала установи Fabric для этой сборки."); return; }
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
      if (contentType === "mod") { setInstalledMods(await invoke<InstalledMod[]>("install_modrinth_mod", { instanceId: selected.id, projectId })); await refreshModUpdates(selected.id); setNotice("Мод и необходимые зависимости установлены и проверены."); }
      else { setInstalledContent(await invoke<InstalledMod[]>("install_modrinth_content", { instanceId: selected.id, projectId, contentType })); setNotice(contentType === "shader" ? "Шейдер установлен и проверен." : "Набор текстур установлен и проверен."); }
    } catch (e) { setError(String(e)); }
    finally { setModBusy(""); }
  }

  async function toggleContent(projectId: string, enabled: boolean) {
    if (!selected || modBusy) return;
    setModBusy(projectId); setError(""); setNotice("");
    try {
      setInstalledContent(await invoke<InstalledMod[]>("set_modrinth_content_enabled", { instanceId: selected.id, projectId, contentType, enabled }));
      setNotice(enabled ? "Контент включён. Возможно, потребуется перезагрузить ресурсы Minecraft." : "Контент отключён.");
    } catch (e) { setError(String(e)); }
    finally { setModBusy(""); }
  }

  async function uninstallMod(projectId: string) {
    if (!selected || modBusy) return;
    setModBusy(projectId); setError(""); setNotice("");
    try {
      if (contentType === "mod") { setInstalledMods(await invoke<InstalledMod[]>("uninstall_modrinth_mod", { instanceId: selected.id, projectId })); await refreshModUpdates(selected.id); }
      else setInstalledContent(await invoke<InstalledMod[]>("uninstall_modrinth_content", { instanceId: selected.id, projectId, contentType }));
      setNotice("Контент удалён.");
    } catch (e) { setError(String(e)); }
    finally { setModBusy(""); }
  }

  const selected = instances.find((item) => item.id === selectedId);

  async function createInstance() {
    if (!newName.trim() || !newVersion) return;
    setBusy(true); setInstalling(true); setError(""); setNotice("");
    try {
      const created = await invoke<Instance>("create_instance", { name: newName, gameVersion: newVersion });
      setInstances((current) => [...current, created]);
      setSelectedId(created.id); setNewName(""); setCreating(false); setInstalled(false);
      setNotice(`Сборка «${created.name}» создана. Загружаем Minecraft ${created.gameVersion}…`);
      const result = await invoke<InstallResult>("install_game", { instanceId: created.id });
      await refreshJavaRuntimes();
      setInstalled(true);
      setNotice(`Minecraft ${result.version} установлен. Библиотек загружено: ${result.librariesDownloaded}; игровых ресурсов: ${result.assetsDownloaded}.`);
    } catch (e) { setError(`Не удалось создать или установить сборку: ${String(e)}`); await refreshInstances(); }
    finally { setBusy(false); setInstalling(false); }
  }

  async function installSelectedGame() {
    if (!selected || installing) return;
    setInstalling(true); setError(""); setNotice("");
    try {
      const result = await invoke<InstallResult>("install_game", { instanceId: selected.id });
      await refreshJavaRuntimes();
      setInstalled(true);
      setNotice(`Minecraft ${result.version} установлен и проверен. Библиотек загружено: ${result.librariesDownloaded}; ресурсов: ${result.assetsDownloaded}.`);
    } catch (e) { setError(String(e)); }
    finally { setInstalling(false); }
  }

  async function verifyAssets() {
    if (!selected || verifyingAssets) return;
    setVerifyingAssets(true); setError(""); setNotice("");
    try {
      const result = await invoke<AssetVerification>("verify_asset_cache", { instanceId: selected.id });
      setAssetVerification(result);
      setNotice(result.missing === 0 && result.corrupt === 0
        ? `Кэш ресурсов проверен: ${result.verified.toLocaleString()} объектов, потоков Rust: ${result.workersUsed}.`
        : `Кэш ресурсов требует восстановления: отсутствует ${result.missing}, повреждено ${result.corrupt}.`);
    } catch (e) { setError(String(e)); }
    finally { setVerifyingAssets(false); }
  }

  async function repairAssets() {
    if (!selected || repairingAssets) return;
    setRepairingAssets(true); setError(""); setNotice("");
    try {
      const result = await invoke<AssetRepairResult>("repair_asset_cache", { instanceId: selected.id });
      setAssetVerification(null);
      setNotice(`Кэш ресурсов восстановлен: загружено ${result.downloaded.toLocaleString()}, уже в порядке ${result.alreadyValid.toLocaleString()}.`);
    } catch (e) { setError(String(e)); }
    finally { setRepairingAssets(false); }
  }

  async function installFabric() {
    if (!selected || installingFabric || !installed) return;
    setInstallingFabric(true); setError(""); setNotice("");
    try {
      const result = await invoke<{ loader: string; librariesDownloaded: number; librariesAlreadyPresent: number }>("install_fabric", { instanceId: selected.id });
      await refreshInstances();
      setNotice(`${result.loader} установлен. Библиотек загружено: ${result.librariesDownloaded}; уже было: ${result.librariesAlreadyPresent}.`);
    } catch (e) { setError(String(e)); }
    finally { setInstallingFabric(false); }
  }

  async function saveRename(id: string) {
    setBusy(true); setError("");
    try {
      await invoke<Instance>("rename_instance", { id, name: editName });
      await refreshInstances(); setEditingId(null); setNotice("Сборка переименована.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  async function removeInstance(item: Instance) {
    if (!window.confirm(`Удалить сборку «${item.name}» и все её файлы?`)) return;
    setBusy(true); setError("");
    try { await invoke("delete_instance", { id: item.id }); await refreshInstances(); setNotice("Сборка удалена."); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand"><div className="brand-mark">C</div><div><strong>ComfyCraft</strong><span>JAVA EDITION</span></div></div>
        <div className="nav-label">РАБОЧАЯ ОБЛАСТЬ</div>
        <nav>{nav.map(({ label, icon: Icon }) => <button key={label} className={page === label ? "nav-item active" : "nav-item"} onClick={() => setPage(label)}><Icon size={18}/>{label === "Play" ? "Играть" : label === "Instances" ? "Сборки" : label === "Mods" ? "Моды" : "Настройки"}</button>)}</nav>
        <div className="sidebar-bottom"><div className="linux-pill"><span/> LINUX ДЛЯ ПК</div><small>Ранняя разработка · v0.1.0</small></div>
      </aside>
      <section className="content">
        <header className="topbar"><div className="breadcrumb">ComfyCraft <span>/</span> {page === "Play" ? "Играть" : page === "Instances" ? "Сборки" : page === "Mods" ? "Моды" : "Настройки"}</div><div className="top-status"><span className="status-dot"/> Хранилище сборок готово</div></header>
        {(error || notice) && <div className={error ? "notice error" : "notice"}>{error || notice}<button onClick={() => { setError(""); setNotice(""); }}>×</button></div>}
        {page === "Play" && <div className="page play-page">
          <div className="eyebrow"><Sparkles size={14}/> ТВОЙ MINECRAFT</div>
          <h1>Твой мир.<br/>Твои правила.</h1>
          <p className="lead">Миры, моды и настройки — в одном месте.</p>
          {!selected && <form className="create-panel first-build-panel" onSubmit={(e) => { e.preventDefault(); void createInstance(); }}>
            <div className="build-form-title"><b>Создать первую сборку</b><span>Выбери версию Minecraft — лаунчер создаст профиль и сразу начнёт установку.</span></div>
            <label>НАЗВАНИЕ СБОРКИ<input autoFocus maxLength={40} value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="Моё выживание"/></label>
            <label>ВЕРСИЯ MINECRAFT<select value={newVersion} onChange={(e) => setNewVersion(e.target.value)} disabled={versionsLoading || gameVersions.length === 0}>
              {versionsLoading && <option value="">Загружаем список версий…</option>}
              {gameVersions.map((version) => <option key={version.id} value={version.id}>{version.id}</option>)}
            </select></label>
            <button className="primary-button build-create-button" disabled={busy || installing || !newName.trim() || !newVersion}><Download size={16}/>{busy || installing ? "Создаём и устанавливаем…" : "Создать сборку и скачать Minecraft"}</button>
            {versionsError && <p className="version-error">Не удалось получить список версий: {versionsError}</p>}
          </form>}
          {selected && <div className="build-toolbar"><span>Выбрана сборка: <b>{selected.name}</b></span><button className="outline-button" onClick={() => setCreating((v) => !v)}><Plus size={15}/> Новая сборка</button></div>}
          {selected && creating && <form className="create-panel" onSubmit={(e) => { e.preventDefault(); void createInstance(); }}>
            <label>НАЗВАНИЕ СБОРКИ<input autoFocus maxLength={40} value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="Моё выживание"/></label>
            <label>ВЕРСИЯ MINECRAFT<select value={newVersion} onChange={(e) => setNewVersion(e.target.value)} disabled={versionsLoading || gameVersions.length === 0}>
              {versionsLoading && <option value="">Загружаем версии…</option>}
              {gameVersions.map((version) => <option key={version.id} value={version.id}>{version.id}</option>)}
            </select></label>
            <button className="primary-button" disabled={busy || installing || !newName.trim() || !newVersion}><Plus size={15}/> Создать и установить</button>
          </form>}
          {selected && <div className="launch-card">
            <div className="card-top"><div className="game-icon"><Box size={25}/></div><div className="instance-info"><span>ВЫБРАННАЯ СБОРКА</span><strong>{selected?.name ?? "Сборка не выбрана"}</strong><small>{selected ? `Minecraft Java Edition · ${selected.gameVersion} · ${selected.loader}` : "Создай сборку, чтобы начать"}</small></div>
              <select aria-label="Выбрать сборку" className="instance-select" value={selectedId} onChange={(e) => setSelectedId(e.target.value)}><option value="">Выбрать сборку</option>{instances.map((item) => <option key={item.id} value={item.id}>{item.name}</option>)}</select>
            </div>
            <div className="card-divider"/>
            <div className="launch-meta"><div><span>ЗАГРУЗЧИК</span><b>{selected?.loader ?? "—"}</b></div><div><span>JAVA</span><b>Требуется Java 25</b></div><div><span>ПРОФИЛЬ</span><b>Производительность</b></div></div>
            <button className="play-button" onClick={() => void installSelectedGame()} disabled={installing || busy}>
              {installed ? <CheckCircle2 size={17}/> : <Download size={17}/>}
              {installing ? "УСТАНОВКА…" : installed ? "ПРОВЕРИТЬ ФАЙЛЫ ИГРЫ" : "СКАЧАТЬ MINECRAFT"}
              <span>{installing ? "Загружаем официальные файлы" : installed ? "Файлы установлены" : "Клиент · библиотеки · ресурсы"}</span>
            </button>
            {installed && <button className="outline-button readiness-button" disabled={installingFabric || installing} onClick={() => void installFabric()}><Blocks size={15}/>{installingFabric ? "УСТАНАВЛИВАЕМ FABRIC…" : selected?.loader.startsWith("Fabric ") ? "ВОССТАНОВИТЬ / ОБНОВИТЬ FABRIC" : "УСТАНОВИТЬ FABRIC"}<span>Последняя совместимая версия загрузчика</span></button>}
            {installed && <button className="outline-button readiness-button" disabled={checkingLaunch} onClick={async () => {
              if (!selected) return;
              setCheckingLaunch(true); setError(""); setNotice("");
              try {
                const result = await invoke<Readiness>("check_launch_readiness", { instanceId: selected.id });
                setReadiness(result);
                const inspection = await invoke<LaunchInspection>("inspect_launch_arguments", { instanceId: selected.id });
                setLaunchInspection(inspection);
                setNotice(result.ready ? "Проверки системы пройдены. Для запуска ещё нужна авторизация Ely.by." : "Некоторые системные проверки не пройдены.");
              } catch (e) { setError(String(e)); }
              finally { setCheckingLaunch(false); }
            }}><RefreshCw size={15}/>{checkingLaunch ? "ПРОВЕРЯЕМ…" : "ПРОВЕРИТЬ ГОТОВНОСТЬ К ЗАПУСКУ"}</button>}
                         {installed && <button className="outline-button readiness-button" disabled={verifyingAssets || installing} onClick={() => void verifyAssets()}><RefreshCw size={15}/>{verifyingAssets ? "ПРОВЕРЯЕМ РЕСУРСЫ…" : "ПРОВЕРИТЬ КЭШ РЕСУРСОВ"}<span>Параллельная проверка SHA-1 · потоки Rust</span></button>}
                          {installed && <button className="outline-button readiness-button" disabled={repairingAssets || installing} onClick={() => void repairAssets()}><Download size={15}/>{repairingAssets ? "ВОССТАНАВЛИВАЕМ РЕСУРСЫ…" : "ВОССТАНОВИТЬ КЭШ РЕСУРСОВ"}<span>Загрузка отсутствующих и повреждённых файлов с проверкой SHA-1</span></button>}
             {assetVerification && <div className="readiness-list"><div><span className={assetVerification.missing === 0 && assetVerification.corrupt === 0 ? "status-dot" : "status-dot status-fail"}/>Объекты ресурсов<b>{assetVerification.verified.toLocaleString()} / {assetVerification.total.toLocaleString()}</b></div><div><span className="status-dot"/>Потоки Rust<b>{assetVerification.workersUsed}</b></div><div><span className={assetVerification.missing || assetVerification.corrupt ? "status-dot status-fail" : "status-dot"}/>Отсутствуют / повреждены<b>{assetVerification.missing} / {assetVerification.corrupt}</b></div><small>{(assetVerification.bytesVerified / 1024 / 1024).toFixed(1)} МиБ проверено за {assetVerification.elapsedMs} ms · индекс {assetVerification.indexId}</small>{assetVerification.problemSamples.length > 0 && <details className="argument-preview"><summary>Проблемы проверки</summary><pre>{assetVerification.problemSamples.join("\n")}</pre></details>}</div>}
             {readiness && <div className="readiness-list">{readiness.checks.map((check) => <div key={check.id}><span className={check.ok ? "status-dot" : "status-dot status-fail"}/>{check.label}<b>{check.ok ? "ГОТОВО" : "НЕТ"}</b></div>)}<small>{readiness.authentication}</small></div>}
            {launchInspection && <div className="readiness-list"><div><span className={launchInspection.javaRuntime.available && (launchInspection.javaRuntime.major ?? 0) >= launchInspection.requiredJavaMajor ? "status-dot" : "status-dot status-fail"}/>Среда Java<b>{launchInspection.javaRuntime.version ?? "Не найдена"} · требуется {launchInspection.requiredJavaMajor}+</b></div><div><span className="status-dot"/>Параметры JVM<b>{launchInspection.jvmArgumentCount}</b></div><div><span className="status-dot"/>Параметры игры<b>{launchInspection.gameArgumentCount}</b></div><div><span className={launchInspection.unresolvedPlaceholders.length ? "status-dot status-fail" : "status-dot"}/>Необработанные параметры<b>{launchInspection.unresolvedPlaceholders.length}</b></div><small>{launchInspection.note}</small><details className="argument-preview"><summary>Параметры JVM</summary><pre>{launchInspection.jvmArguments.join("\n") || "Нет"}</pre></details><details className="argument-preview"><summary>Параметры запуска игры</summary><pre>{launchInspection.gameArguments.join("\n") || "Нет"}</pre></details></div>}
            <p className="hint">Файлы загружаются с официальных серверов Mojang и проверяются по SHA-1 и размеру.</p>
          </div>}
          <div className="quick-grid"><div className="quick-card"><Blocks/><div><b>Менеджер модов</b><span>Modrinth · зависимости Fabric</span></div></div><div className="quick-card"><Monitor/><div><b>Графика</b><span>Iris, шейдеры и наборы текстур</span></div></div></div>
        </div>}
        {page === "Instances" && <div className="page">
          <div className="page-heading"><div><div className="eyebrow">ТВОИ СБОРКИ</div><h2>Сборки</h2><p>Отдельные установки Minecraft. У каждой сборки свои моды, миры и настройки.</p></div><button className="outline-button" onClick={() => setCreating((v) => !v)}><Plus size={16}/> Новая сборка</button></div>
          {creating && <form className="create-panel" onSubmit={(e) => { e.preventDefault(); void createInstance(); }}>
            <label>НАЗВАНИЕ СБОРКИ<input autoFocus maxLength={40} value={newName} onChange={(e) => setNewName(e.target.value)} placeholder="Моё выживание"/></label>
            <label>ВЕРСИЯ MINECRAFT<select value={newVersion} onChange={(e) => setNewVersion(e.target.value)} disabled={versionsLoading || gameVersions.length === 0}>
              {versionsLoading && <option value="">Загружаем официальные версии…</option>}
              {gameVersions.map((version) => <option key={version.id} value={version.id}>{version.id}</option>)}
            </select></label>
            <button className="primary-button" disabled={busy || !newName.trim() || !newVersion}><Plus size={16}/> Создать сборку</button>
            {versionsError && <p className="version-error">{versionsError}</p>}
          </form>}
          {instances.length === 0 ? <div className="empty-state"><Box size={30}/><h3>Сборок пока нет</h3><p>Создай свою первую сборку Minecraft.</p></div> :
            <div className="instance-list">{instances.map((item) => <article className={selectedId === item.id ? "instance-row selected" : "instance-row"} key={item.id}>
              <div className="game-icon"><Box size={22}/></div><div className="instance-row-info">{editingId === item.id ? <form className="rename-form" onSubmit={(e) => { e.preventDefault(); void saveRename(item.id); }}><input autoFocus maxLength={40} value={editName} onChange={(e) => setEditName(e.target.value)}/><button className="text-action">Сохранить</button><button type="button" className="text-action" onClick={() => setEditingId(null)}>Отмена</button></form> : <><b>{item.name}</b><span>Minecraft {item.gameVersion} · {item.loader}</span></>}</div>
              <button className="text-action" onClick={() => setSelectedId(item.id)}>{selectedId === item.id ? "Выбрана" : "Выбрать"}</button>
              <button className="icon-action" aria-label="Переименовать сборку" onClick={() => { setEditingId(item.id); setEditName(item.name); }}><Pencil size={15}/></button>
              <button className="icon-action danger" aria-label="Удалить сборку" onClick={() => void removeInstance(item)} disabled={busy}><Trash2 size={15}/></button>
            </article>)}</div>}
          <button className="refresh-button" onClick={() => void refreshInstances()}><RefreshCw size={14}/> Обновить список</button>
        </div>}
        {page === "Mods" && <div className="page">
          <div className="eyebrow">MODRINTH · БИБЛИОТЕКА КОНТЕНТА</div><h2>Моды и графика</h2>
          <p>Устанавливай совместимые моды Fabric, наборы текстур и шейдеры в выбранную сборку.</p>
          <div className="content-tabs">
            {([["mod","Моды"],["resourcepack","Наборы текстур"],["shader","Шейдеры"]] as [ContentType,string][]).map(([type,label]) => <button key={type} className={contentType === type ? "content-tab active" : "content-tab"} onClick={() => setContentType(type)}>{label}</button>)}
          </div>
          <div className="mod-search"><input value={modQuery} onChange={(e) => setModQuery(e.target.value)} onKeyDown={(e) => { if (e.key === "Enter") void searchMods(); }} placeholder={contentType === "mod" ? "Поиск модов на Modrinth…" : contentType === "shader" ? "Поиск шейдеров…" : "Поиск наборов текстур…"} aria-label="Поиск контента Modrinth"/><button className="primary-button" onClick={() => void searchMods()} disabled={searchingMods || !selected || (contentType === "mod" && !selected.loader.startsWith("Fabric "))}><Search size={15}/>{searchingMods ? "Ищем…" : "Искать"}</button></div>
          {!selected ? <div className="empty-state"><Box size={28}/><h3>Выбери сборку</h3><p>Сначала выбери сборку, чтобы просматривать контент.</p></div> : contentType === "mod" && !selected.loader.startsWith("Fabric ") ? <div className="empty-state"><Blocks size={28}/><h3>Требуется Fabric</h3><p>Установи Minecraft и Fabric версии {selected.gameVersion} на вкладке «Играть», чтобы просматривать моды.</p></div> : null}
          {modResults.length > 0 && <div className="mod-list">{modResults.map((item) => {
            const managed = contentType === "mod" ? installedMods.some((entry) => entry.projectId === item.projectId) : installedContent.some((entry) => entry.projectId === item.projectId);
            return <article className="mod-row" key={item.projectId}>
              {item.iconUrl ? <img className="mod-icon" src={item.iconUrl} alt="" /> : <div className="mod-icon fallback"><Blocks size={20}/></div>}
              <div className="mod-info"><b>{item.title}</b><span>Автор: {item.author} · загрузок: {item.downloads.toLocaleString()}</span><p>{item.description}</p></div>
              <button className={managed ? "outline-button" : "primary-button"} disabled={Boolean(modBusy)} onClick={() => void (managed ? uninstallMod(item.projectId) : installMod(item.projectId))}>{modBusy === item.projectId ? "Выполняем…" : managed ? "Удалить" : "Установить"}</button>
            </article>;
          })}</div>}
          {modResults.length === 0 && !searchingMods && selected && (contentType !== "mod" || selected.loader.startsWith("Fabric ")) && <div className="empty-state compact-empty"><Search size={28}/><h3>{contentType === "mod" ? "Найти мод" : contentType === "shader" ? "Найти шейдер" : "Найти набор текстур"}</h3><p>Результаты для Minecraft {selected.gameVersion}{contentType === "mod" ? " и Fabric" : ""}.</p></div>}
          {contentType === "mod" ? <div className="installed-mods"><div className="section-kicker">УСТАНОВЛЕННЫЕ МОДЫ · ДОСТУПНО ОБНОВЛЕНИЙ: {modUpdates.length}</div>{installedMods.length === 0 ? <p className="muted">Управляемые моды ещё не установлены.</p> : installedMods.map((item) => {
            const update = modUpdates.find((entry) => entry.projectId === item.projectId);
            return <div className="installed-mod-row" key={item.projectId}><span>{item.name}</span><small>{item.filename}</small>{update && <button className="text-action" disabled={Boolean(modBusy)} onClick={() => void installMod(item.projectId)}>{modBusy === item.projectId ? "Обновляем…" : "Обновить"}</button>}<button className="text-action" disabled={Boolean(modBusy)} onClick={() => void uninstallMod(item.projectId)}>Удалить</button></div>;
          })}</div> : <div className="installed-mods"><div className="section-kicker">УСТАНОВЛЕНО: {contentType === "shader" ? "ШЕЙДЕРЫ" : "НАБОРЫ ТЕКСТУР"}</div>{installedContent.length === 0 ? <p className="muted">Управляемый контент ещё не установлен.</p> : installedContent.map((item) => <div className="installed-mod-row" key={item.projectId}><span>{item.name}</span><small>{item.active ? "Включено" : "Отключено"} · {item.filename}</small><button className="text-action" disabled={Boolean(modBusy)} onClick={() => void toggleContent(item.projectId, !item.active)}>{modBusy === item.projectId ? "Сохраняем…" : item.active ? "Отключить" : "Включить"}</button><button className="text-action" disabled={Boolean(modBusy)} onClick={() => void uninstallMod(item.projectId)}>Удалить</button></div>)}</div>}
        </div>}
        {page === "Settings" && <div className="page"><div className="eyebrow">ПАРАМЕТРЫ</div><h2>Настройки</h2><p>ComfyCraft разрабатывается для Linux.</p><div className="settings-row"><div><b>Целевая платформа</b><span>CachyOS / Arch Linux · Wayland / X11</span></div><strong>Только Linux</strong></div><div className="settings-row"><div><b>Тестирование графики</b><span>NVIDIA · KDE Plasma · Hyprland</span></div><strong>В планах</strong></div><div className="settings-row"><div><b>Аккаунт</b><span>Авторизация через Ely.by</span></div><strong>В планах</strong></div><div className="settings-row"><div><b>Среда Java</b><span>ComfyCraft автоматически загружает подходящую Java при установке Minecraft.</span></div><div className="settings-actions"><button className="text-action" onClick={() => void refreshJavaRuntimes()} disabled={javaLoading || javaInstalling}><RefreshCw size={14}/>{javaLoading ? "Проверяем…" : "Проверить снова"}</button><button className="text-action" onClick={async () => { setJavaInstalling(true); setError(""); setNotice(""); try { setJavaRuntimes(await invoke<JavaRuntime[]>("install_java_runtime", { majorVersion: 25 })); setNotice("Java 25 загружена, проверена и выбрана."); } catch (e) { setError("Не удалось установить Java: " + String(e)); } finally { setJavaInstalling(false); } }} disabled={javaInstalling || javaLoading}><Download size={14}/>{javaInstalling ? "ЗАГРУЖАЕМ JAVA…" : "СКАЧАТЬ JAVA 25"}</button></div></div>{javaRuntimes.length === 0 ? <div className="empty-state"><Monitor size={25}/><h3>Java не найдена</h3><p>Установи Java для Linux и запусти повторную проверку. ComfyCraft не загружает Java из непроверенных источников.</p></div> : <div className="instance-list">{javaRuntimes.map((runtime) => <article className={runtime.selected ? "instance-row selected" : "instance-row"} key={runtime.executable}><div className="game-icon"><Monitor size={20}/></div><div className="instance-row-info"><b>Java {runtime.major} · {runtime.version}</b><span>{runtime.vendor ?? "Среда Java"} · {runtime.executable}</span></div><strong>{runtime.selected ? "Выбрана" : runtime.major >= 25 ? "Совместима с 26.3" : "Старая версия"}</strong>{!runtime.selected && <button className="text-action" onClick={async () => { try { setJavaRuntimes(await invoke<JavaRuntime[]>("select_java_runtime", { executable: runtime.executable })); setNotice("Выбор версии Java сохранён."); } catch (e) { setError(String(e)); } }}>Использовать</button>}</article>)}</div>}</div>}
      </section>
    </main>
  );
}
export default App;
