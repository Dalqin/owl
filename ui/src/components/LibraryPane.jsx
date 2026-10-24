import { unknown } from "../unknown.js";

// How each kind of entry in a library is described.
function status(entry) {
  switch (entry.kind) {
    case "mod":
      if (entry.added) return "On your list";
      return entry.in_cfg ? "Already in your OpenMW settings, not added by Owl" : "Ready to add";
    case "archive":
      return "An archive. Extract it first, then choose the extracted folder.";
    case "other":
      return "No mod found in this folder.";
    default:
      return unknown("library entry kind", entry.kind);
  }
}

// A folder of mods: what is in it, and which of them are already on the list.
export default function LibraryPane({ library, onOpen }) {
  const name = library.path.split(/[\\/]/).filter(Boolean).pop() ?? library.path;
  const left = library.entries.filter((e) => e.kind === "mod" && !e.added).length;

  return (
    <section aria-label={`Library ${name}`}>
      <h2>{name}</h2>
      <p className="muted path">{library.path}</p>

      {!library.exists ? (
        <p className="note" role="note">This folder is not there any more. It may be on a drive that is not connected.</p>
      ) : library.entries.length === 0 ? (
        <p className="muted">Nothing in this folder.</p>
      ) : (
        <>
          {left === 0 && <p>Every mod in this folder is on your list.</p>}
          <ul className="library-entries">
            {library.entries.map((entry, i) => (
              <li key={`${i}-${entry.path}`} className={`library-entry library-${entry.kind}`}>
                <div className="library-text">
                  <span className="library-name">{entry.name}</span>
                  <span className="muted library-status">{status(entry)}</span>
                </div>
                {entry.kind === "mod" && !entry.added && (
                  <button aria-label={`Open ${entry.name}`} onClick={() => onOpen(entry.path)}>
                    Open
                  </button>
                )}
              </li>
            ))}
          </ul>
          {library.truncated && <p className="muted">Only the first part of this folder is listed.</p>}
        </>
      )}
    </section>
  );
}
