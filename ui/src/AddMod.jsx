import { useEffect, useRef, useState } from "react";

import * as api from "./api.js";

// Choose a folder. "+ Add a mod" opens the system's chooser straight away (`pick`
// changes on every click, so choosing again always asks again). A folder that is
// one mod joins "Individual mods" and opens at once; a folder that holds many
// mods is offered as a library whose mods can be worked through one by one.
export default function AddMod({ pick, onIndividualAdded, onLibraryAdded }) {
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

  // Remembered until it is added, so it shows in the list of mods available.
  async function useAsMod(path) {
    await api.addIndividual(path);
    setPending(null);
    await onIndividualAdded(path);
  }

  async function useAsLibrary() {
    setError(null);
    setBusy(true);
    try {
      await api.addLibrary(pending);
      await onLibraryAdded(pending);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section aria-label="Add a mod">
      <p className="muted">
        {busy
          ? "Looking…"
          : "Choose one mod's folder, or a folder that holds many mods and work through them. Use “+ Add a mod” to choose a folder."}
      </p>

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
    </section>
  );
}
