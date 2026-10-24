// For a mod with several possible folders and no installer: which ones to use.
// Mods that ship numbered parts ("00 Core", "01 Patch") are bundles, so any
// number can be on. They stay in folder order, which is the order they load in.
export default function DatapathPicker({ candidates, chosen, onChoose }) {
  function toggle(path) {
    const on = chosen.includes(path);
    onChoose(candidates.map((c) => c.path).filter((p) => (p === path ? !on : chosen.includes(p))));
  }

  return (
    <section aria-labelledby="folders-heading">
      <h3 id="folders-heading">Which folders do you want?</h3>
      <div role="group" aria-labelledby="folders-heading">
        {candidates.map((c) => {
          const label = c.path === "" ? "(this folder)" : c.path;
          const plugins = c.plugins.map((p) => p.name).join(", ");
          const on = chosen.includes(c.path);
          return (
            <label key={c.path} className={`option ${on ? "selected" : ""}`}>
              <input type="checkbox" checked={on} onChange={() => toggle(c.path)} />
              <span>
                <span className="option-name">{label}</span>
                {plugins && <span className="muted option-desc">Plugins: {plugins}</span>}
              </span>
            </label>
          );
        })}
      </div>
    </section>
  );
}
