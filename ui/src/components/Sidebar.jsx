import { FolderIcon, HelpIcon, ModIcon, PickedIcon } from "./icons.jsx";

// The master list, top to bottom as the work goes: adding a mod, the mods available
// (picked one at a time, then folders of mods), the mods on the list in load order,
// and the review.
// `selection` is { kind: "add" } | { kind: "mod", index } | { kind: "list" }
//   | { kind: "library", path } | { kind: "candidate", path } | { kind: "help" }.
export default function Sidebar({
  mods,
  check,
  libraries = [],
  individuals = [],
  selection,
  onSelect,
  onForgetLibrary = () => {},
  onForgetIndividual = () => {},
}) {
  const current = (kind, key) => {
    if (selection.kind !== kind) return undefined;
    switch (kind) {
      case "mod":
        return selection.index === key ? "page" : undefined;
      case "library":
      case "candidate":
        return selection.path === key ? "page" : undefined;
      default:
        return "page";
    }
  };
  const baseName = (path) => path.split(/[\\/]/).filter(Boolean).pop() ?? path;

  return (
    <nav aria-label="Your mods" className="sidebar">
      <button className="side-item side-add" aria-current={current("add")} onClick={() => onSelect({ kind: "add", pick: Date.now() })}>
        + Add a mod
      </button>

      {(individuals.length > 0 || libraries.length > 0) && (
        <div className="side-sources">
          <p className="muted side-hint">Available to add</p>

          {individuals.length > 0 && (
            <div className="side-source">
              <div className="side-group">
                <PickedIcon />
                <span className="side-name">
                  Individual mods
                  <span className="side-sub">Not part of a folder</span>
                </span>
                <span className="side-count">{individuals.length}</span>
              </div>
              <ul className="side-candidates">
                {individuals.map((m) => (
                  <li key={m.path} className="side-row">
                    <button
                      className="side-item"
                      aria-current={current("candidate", m.path)}
                      onClick={() => onSelect({ kind: "candidate", path: m.path })}
                    >
                      <ModIcon />
                      <span className="side-name">{m.name}</span>
                      {!m.exists && <span className="side-count">missing</span>}
                    </button>
                    <button className="side-forget" aria-label={`Forget ${m.name}`} title="Forget this mod without adding it" onClick={() => onForgetIndividual(m.path)}>
                      ✕
                    </button>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {libraries.map((lib) => {
            const left = lib.entries.filter((e) => e.kind === "mod" && !e.added);
            return (
              <div key={lib.path} className="side-source">
                <div className="side-row">
                  <button
                    className="side-item side-lib"
                    aria-current={current("library", lib.path)}
                    onClick={() => onSelect({ kind: "library", path: lib.path })}
                  >
                    <FolderIcon />
                    <span className="side-name">{baseName(lib.path)}</span>
                    <span className="side-count">{lib.exists ? left.length : "missing"}</span>
                  </button>
                  <button className="side-forget" aria-label={`Stop listing ${baseName(lib.path)}`} title="Remove this source of mods" onClick={() => onForgetLibrary(lib.path)}>
                    ✕
                  </button>
                </div>
                <ul className="side-candidates">
                  {left.map((e) => (
                    <li key={e.path}>
                      <button
                        className="side-item"
                        aria-current={current("candidate", e.path)}
                        onClick={() => onSelect({ kind: "candidate", path: e.path })}
                      >
                        <ModIcon />
                        <span className="side-name">{e.name}</span>
                      </button>
                    </li>
                  ))}
                </ul>
              </div>
            );
          })}
        </div>
      )}

      {mods.length > 0 && <p className="muted side-hint">Mods load in this order. Later ones win where they overlap.</p>}
      <ol className="side-mods">
        {mods.map((m, i) => {
          const problem = check.mods[i].issues.length > 0 || !m.root_exists;
          return (
            <li key={`${i}-${m.root}`}>
              <button
                className="side-item"
                aria-current={current("mod", i)}
                onClick={() => onSelect({ kind: "mod", index: i })}
              >
                <span className="side-num">{i + 1}.</span>
                <ModIcon />
                <span className="side-name">{m.manifest.name}</span>
                {problem && (
                  <span className="side-flag" role="img" aria-label="has problems">
                    ⚠
                  </span>
                )}
              </button>
            </li>
          );
        })}
      </ol>

      <button className="side-item side-whole" aria-current={current("list")} onClick={() => onSelect({ kind: "list" })}>
        <span className="side-name">Review and finalize</span>
        {check.issues.length > 0 && (
          <span className="side-flag" role="img" aria-label={`${check.issues.length} problems`}>
            ⚠ {check.issues.length}
          </span>
        )}
      </button>

      <button className="side-item" aria-current={current("help")} onClick={() => onSelect({ kind: "help" })}>
        <HelpIcon />
        <span className="side-name">Help</span>
      </button>
    </nav>
  );
}
