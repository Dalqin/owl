import { useEffect, useRef, useState } from "react";

import * as api from "./api.js";

// Choose a folder. "Add a source" opens the system's chooser (`pick` changes on
// every click, so choosing again always asks again). A folder that is one mod
// becomes a source; a folder that holds many mods is offered as a source whose
// mods can be worked through one by one. Either way the player stays where they are.
export default function AddMod({ pick, onSourceAdded }) {
  const [pending, setPending] = useState(null); // a folder of many mods, waiting for the player's choice
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);

  // Each click asks once, even where the effect runs twice (React's strict mode).
  const asked = useRef(null);
  useEffect(() => {
    if (pick == null || asked.current === pick) return;
    asked.current = pick;
    pickFolder();
  }, [pick]);

  async function pickFolder() {
    setError(null);
    let path;
    try {
      path = await api.chooseFolder();
    } catch (e) {
      // The system's chooser could not open (for example no desktop portal is running).
      setError(`The folder chooser could not open: ${String(e)}`);
      return;
    }
    if (!path) return;
    setBusy(true);
    try {
      const kind = await api.classifyFolder(path);
      if (kind === "mod") {
        await useAsMod(path);
      } else {
        setPending(path);
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  // Remembered as a source until it is added, so it shows in the list of mods available.
  async function useAsMod(path) {
    const added = await api.addSource(path, "mod");
    setPending(null);
    await onSourceAdded(path, added);
  }

  async function useAsLibrary() {
    setError(null);
    setBusy(true);
    try {
      const path = pending;
      const added = await api.addSource(path, "library");
      setPending(null);
      await onSourceAdded(path, added);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div aria-label="Add a source">
      {busy && <p className="muted">Looking…</p>}

      {error && <p className="note error" role="alert">{error}</p>}

      {pending && (
        <div className="library-choice" role="group" aria-label="This folder holds several mods">
          <h2>This folder holds several mods</h2>
          <p className="path">{pending}</p>
          <p>Show its mods in your list, and add the ones you want one at a time?</p>
          <div className="actions">
            <button onClick={useAsLibrary} disabled={busy}>Show its mods in my list</button>
            <button
              className="secondary"
              onClick={async () => {
                setError(null);
                setBusy(true);
                try {
                  await useAsMod(pending);
                } catch (e) {
                  setError(String(e));
                } finally {
                  setBusy(false);
                }
              }}
              disabled={busy}
            >
              It is one mod
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
