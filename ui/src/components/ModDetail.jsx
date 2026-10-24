import { useState } from "react";

import * as api from "../api.js";
import { describeDecisionIssue, describeIssue } from "../describe.js";
import ModWorkspace from "./ModWorkspace.jsx";

// A mod on the player's list: change its choices, move it, remove it.
// `onReload` re-reads the list and returns it; `onSelect` moves the selection.
export default function ModDetail({ mod, modCheck, index, count, cfgPath, onReload, onSelect, onNotice, onToast }) {
  const [error, setError] = useState(null);
  const { manifest } = mod;
  const needsPicker = !manifest.derived.fomod && manifest.derived.datapath_candidates.length > 1;

  async function change(action, then) {
    setError(null);
    try {
      await action();
      await onReload();
      then();
    } catch (e) {
      setError(String(e));
    }
  }

  // Reads the folder again, so a mod added before the scanner knew about something
  // (or updated since) shows it. The player's choices are kept; the ones that no
  // longer match are listed rather than dropped.
  async function reread() {
    setError(null);
    try {
      const issues = await api.rescanMod(index);
      await onReload();
      if (issues.length === 0) {
        onNotice(null);
        onToast("Refreshed");
      } else {
        onNotice(
          `Read ${manifest.name}'s folder again. ${issues.length === 1 ? "One of your choices no longer matches" : `${issues.length} of your choices no longer match`} it: ${issues.map(describeDecisionIssue).join(" ")}`,
        );
      }
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <ModWorkspace
      title={manifest.name}
      folder={mod.root}
      folderMissing={!mod.root_exists}
      fomod={manifest.derived.fomod}
      candidates={manifest.derived.datapath_candidates}
      initial={{
        choices: manifest.decisions.fomod_choices,
        datapaths: needsPicker ? manifest.decisions.datapaths : [],
      }}
      loadPreview={({ choices, datapaths }) =>
        api.previewStoredMod({
          manifest,
          fomodChoices: choices,
          datapaths,
          plugins: [],
          cfgPath,
        })
      }
      saveLabel="Save changes"
      onSave={(saved) => api.addToProfile(saved, mod.root).then(onReload)}
      actions={({ changed }) => (
        <div className="actions">
          <button
            className="secondary"
            aria-label={`Move ${manifest.name} up`}
            disabled={changed || index === 0}
            onClick={() => change(() => api.moveInProfile(index, index - 1), () => onSelect({ kind: "mod", index: index - 1 }))}
          >
            Move up
          </button>
          <button
            className="secondary"
            aria-label={`Move ${manifest.name} down`}
            disabled={changed || index === count - 1}
            onClick={() => change(() => api.moveInProfile(index, index + 1), () => onSelect({ kind: "mod", index: index + 1 }))}
          >
            Move down
          </button>
          <button
            className="secondary"
            aria-label={`Remove ${manifest.name}`}
            disabled={changed}
            onClick={() => change(() => api.removeFromProfile(index), () => onSelect({ kind: "list" }))}
          >
            Remove
          </button>
          <button
            className="secondary"
            title="Read the mod's folder again"
            disabled={changed}
            onClick={reread}
          >
            Refresh
          </button>
        </div>
      )}
    >
      {error && <p className="note error" role="alert">{error}</p>}
      {modCheck.issues.map((issue, i) => (
        <p key={i} className="note" role="note">{describeIssue(issue)}</p>
      ))}
    </ModWorkspace>
  );
}
