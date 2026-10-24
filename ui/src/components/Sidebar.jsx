import { FolderIcon, HelpIcon, ModIcon } from "./icons.jsx";

// The master list, top to bottom as the work goes: managing sources, the sources (folders
// of mods, and single mods not added yet) with what can still be added from them, the
// mods on the list in load order, and the review.
// `selection` is { kind: "sources" } | { kind: "mod", index } | { kind: "list" }
//   | { kind: "library", path } | { kind: "candidate", path } | { kind: "help" }.
export default function Sidebar({ mods, check, sources = [], selection, onSelect, onForgetSource = () => {} }) {
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

  return (
    <nav aria-label="Your mods" className="sidebar">
      <button className="side-item side-add" aria-current={current("sources")} onClick={() => onSelect({ kind: "sources" })}>
        Manage my sources
      </button>

      {sources.length > 0 && (
        <div className="side-sources">
          <p className="muted side-hint">Mods you can add, from your sources</p>

          {sources.map((source) => {
            const forget = (
              <button
                className="side-forget"
                aria-label={`Stop listing ${source.name}`}
                title="Stop listing this source. Nothing is deleted."
                onClick={() => onForgetSource(source.path)}
              >
                ✕
              </button>
            );
            if (source.kind === "mod") {
              return (
                <div key={source.path} className="side-source">
                  <div className="side-row">
                    <button
                      className="side-item"
                      aria-current={current("candidate", source.path)}
                      onClick={() => onSelect({ kind: "candidate", path: source.path })}
                    >
                      <ModIcon />
                      <span className="side-name">{source.name}</span>
                      {!source.exists && <span className="side-count">missing</span>}
                    </button>
                    {forget}
                  </div>
                </div>
              );
            }
            const left = source.entries.filter((e) => e.kind === "mod" && !e.added);
            return (
              <div key={source.path} className="side-source">
                <div className="side-row">
                  <button
                    className="side-item side-lib"
                    aria-current={current("library", source.path)}
                    onClick={() => onSelect({ kind: "library", path: source.path })}
                  >
                    <FolderIcon />
                    <span className="side-name">{source.name}</span>
                    <span className="side-count">{source.exists ? left.length : "missing"}</span>
                  </button>
                  {forget}
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
