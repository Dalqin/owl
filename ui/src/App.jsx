import { useCallback, useEffect, useState } from "react";

import * as api from "./api.js";
import AddMod from "./AddMod.jsx";
import ScannedMod from "./ScannedMod.jsx";
import Help from "./components/Help.jsx";
import LibraryPane from "./components/LibraryPane.jsx";
import ModDetail from "./components/ModDetail.jsx";
import Sidebar from "./components/Sidebar.jsx";
import AllModules from "./components/AllModules.jsx";
import { unknown } from "./unknown.js";
import { OwlLogo } from "./components/icons.jsx";

export default function App() {
  const [cfg, setCfg] = useState({ ready: false, path: null, error: null });
  const [view, setView] = useState(null);
  const [loadError, setLoadError] = useState(null);
  const [profileFile, setProfileFile] = useState(null);
  // Bumped on every reload so a screen showing a mod starts again from what was saved.
  const [version, setVersion] = useState(0);
  const [selection, setSelection] = useState(null); // null until the player picks something
  const [notice, setNotice] = useState(null);
  const [toast, setToast] = useState(null); // { text, id }; fades by itself

  useEffect(() => {
    if (!toast) return;
    const timer = setTimeout(() => setToast(null), 2500);
    return () => clearTimeout(timer);
  }, [toast]);

  useEffect(() => {
    api.locateConfig().then(
      (path) => setCfg({ ready: true, path, error: null }),
      (e) => setCfg({ ready: true, path: null, error: String(e) }),
    );
  }, []);

  // Re-reads the player's list and returns it.
  const reload = useCallback(async () => {
    try {
      const fresh = await api.loadProfile(cfg.path);
      setView(fresh);
      setVersion((n) => n + 1);
      setLoadError(null);
      return fresh;
    } catch (e) {
      setLoadError(String(e));
      return null;
    }
  }, [cfg.path]);

  useEffect(() => {
    if (!cfg.ready) return;
    reload();
    api.profilePath().then(setProfileFile, () => {});
  }, [cfg.ready, reload]);

  // With nothing chosen yet: the list if there are mods, otherwise adding one.
  const mods = view?.mods ?? [];
  const chosen = selection ?? { kind: mods.length > 0 ? "list" : "add" };
  const libraries = view?.libraries ?? [];
  const individuals = view?.individuals ?? [];
  const gone =
    (chosen.kind === "mod" && chosen.index >= mods.length) ||
    (chosen.kind === "library" && !libraries.some((l) => l.path === chosen.path));
  const shown = gone ? { kind: "list" } : chosen;

  // A message belongs to the screen it came from, so choosing something else clears it.
  function select(next) {
    setNotice(null);
    setSelection(next);
  }

  async function added(root) {
    const fresh = await reload();
    const at = fresh ? fresh.mods.findIndex((m) => m.root === root) : -1;
    setSelection(at >= 0 ? { kind: "mod", index: at } : { kind: "list" });
  }

  async function libraryAdded(path) {
    await reload();
    select({ kind: "library", path });
  }

  async function forget(path) {
    const index = libraries.findIndex((l) => l.path === path);
    await api.removeLibrary(index);
    await reload();
    select({ kind: "add" });
  }

  // A single mod joins "Individual mods" and opens, so the sidebar shows where it is.
  async function individualAdded(path) {
    await reload();
    select({ kind: "candidate", path });
  }

  async function forgetIndividual(path) {
    await api.removeIndividual(path);
    await reload();
    if (chosen.kind === "candidate" && chosen.path === path) select({ kind: "add" });
  }


  function pane() {
    switch (shown.kind) {
      case "add":
        return <AddMod pick={shown.pick} onIndividualAdded={individualAdded} onLibraryAdded={libraryAdded} />;
      case "candidate":
        return <ScannedMod key={shown.path} folder={shown.path} cfg={cfg} onAdded={added} />;
      case "library": {
        const library = libraries.find((l) => l.path === shown.path);
        return (
          <LibraryPane library={library} onOpen={(path) => select({ kind: "candidate", path })} />
        );
      }
      case "help":
        return <Help onAdd={() => select({ kind: "add", pick: Date.now() })} />;
      case "list":
        return (
          <AllModules
            view={view}
            cfgPath={cfg.path}
            profileFile={profileFile}
            onReload={reload}
            onAdd={() => select({ kind: "add", pick: Date.now() })}
          />
        );
      case "mod":
        return (
          <ModDetail
            key={`${version}:${shown.index}`}
            mod={view.mods[shown.index]}
            modCheck={view.check.mods[shown.index]}
            index={shown.index}
            count={mods.length}
            cfgPath={cfg.path}
            onReload={reload}
            onSelect={select}
            onNotice={setNotice}
            onToast={(text) => setToast({ text, id: Date.now() })}
          />
        );
      default:
        return unknown("selection", shown);
    }
  }

  return (
    <div className="app">
      <header className="app-header">
        <h1><OwlLogo />Owl</h1>
        <p className="muted">Set up Morrowind mods for OpenMW.</p>
        <p className="muted" data-testid="cfg-status">
          {cfg.path ? `Your OpenMW settings: ${cfg.path}` : `Could not find your OpenMW settings. ${cfg.error ?? ""}`}
        </p>
      </header>

      {view ? (
        <main className="shell">
          <Sidebar
            mods={mods}
            check={view.check}
            libraries={libraries}
            individuals={individuals}
            selection={shown}
            onSelect={select}
            onForgetLibrary={forget}
            onForgetIndividual={forgetIndividual}
          />
          <div className="pane">
            {loadError && <p className="note error" role="alert">{loadError}</p>}
            {notice && <p className="note" role="status">{notice}</p>}
            {pane()}
          </div>
        </main>
      ) : (
        <main className="pane">
          {loadError ? <p className="note error" role="alert">{loadError}</p> : <p className="muted">Loading…</p>}
        </main>
      )}
      {toast && (
        <div key={toast.id} className="toast" role="status">
          {toast.text}
        </div>
      )}
    </div>
  );
}
