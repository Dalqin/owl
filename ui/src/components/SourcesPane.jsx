import AddMod from "../AddMod.jsx";

// What a source holds, in words.
function describe(source) {
  if (!source.exists) return "This folder is not there any more. It may be on a drive that is not connected.";
  if (source.kind === "mod") {
    return source.in_cfg ? "One mod, already in your OpenMW settings (not added by Owl)." : "One mod that is not on your list yet.";
  }
  const mods = source.entries.filter((e) => e.kind === "mod");
  const left = mods.filter((e) => !e.added).length;
  const noun = mods.length === 1 ? "1 mod" : `${mods.length} mods`;
  return `${noun} in this folder, ${left} not on your list yet.`;
}

// Where the player sees, adds and removes sources: the places Owl looks for mods.
// `pick` changes each time "Add a source" is pressed, which opens the folder chooser.
export default function SourcesPane({ sources, pick, onPick, onOpen, onForget, onSourceAdded }) {
  return (
    <section aria-label="My sources">
      <h2>My sources</h2>
      <p>
        A source is a place on your computer where Owl looks for mods: a folder that holds many mods (your downloads
        folder, say), or the folder of a single mod. Owl remembers your sources, so the mods in them are still listed
        the next time you open it.
      </p>
      <p className="muted">
        Removing a source only stops Owl listing it. It does not delete any files, change your OpenMW settings
        (<code>openmw.cfg</code>), or take mods off your list.
      </p>

      <div className="actions">
        <button onClick={onPick}>Add a source</button>
      </div>
      <AddMod pick={pick} onSourceAdded={onSourceAdded} />

      {sources.length === 0 ? (
        <p className="muted">You have no sources yet. Add one to see its mods here.</p>
      ) : (
        <ul className="library-entries">
          {sources.map((source) => (
            <li key={source.path} className="library-entry">
              <div className="library-text">
                <span className="library-name">{source.name}</span>
                <span className="muted path">{source.path}</span>
                <span className="muted library-status">{describe(source)}</span>
              </div>
              {source.exists && (
                <button
                  className="secondary"
                  aria-label={`Open ${source.name}`}
                  onClick={() => onOpen(source.kind === "library" ? { kind: "library", path: source.path } : { kind: "candidate", path: source.path })}
                >
                  Open
                </button>
              )}
              <button className="secondary" aria-label={`Remove source ${source.name}`} onClick={() => onForget(source.path)}>
                Remove
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
