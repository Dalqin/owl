import { useEffect, useState } from "react";

import { describeWarning } from "../describe.js";
import { unknown } from "../unknown.js";
import AuthorNotes from "./AuthorNotes.jsx";
import DatapathPicker from "./DatapathPicker.jsx";
import FomodWizard, { questionId } from "./FomodWizard.jsx";
import ModImage from "./ModImage.jsx";
import Outcome from "./Outcome.jsx";
import Requirements from "./Requirements.jsx";

const TABS = [
  { id: "setup", label: "Setup" },
  { id: "notes", label: "Author's notes" },
  { id: "needs", label: "Needs" },
];

// Where a mod gets set up: its choices, what they lead to, and what the mod
// needs. The same screen serves a mod being added and a mod already on the
// list, so the two cannot drift apart.
//
//   loadPreview({ choices, datapaths })  asks the core what those answers lead to
//   initial                             { choices, datapaths } the screen starts from
//   saveLabel / onSave(manifest)        keep the current answers
//   actions({ changed })                extra buttons (move, remove) for the header; told if there are unsaved answers
//
// A mod already on the list is only saved when its answers changed; a new mod
// can be saved as soon as a folder is decided.
export default function ModWorkspace({
  title,
  folder,
  folderMissing,
  fomod,
  candidates,
  warnings = [],
  initial,
  loadPreview,
  newMod,
  saveLabel,
  onSave,
  actions,
  children,
}) {
  const [tab, setTab] = useState("setup");
  const [choices, setChoices] = useState(initial.choices);
  const [datapaths, setDatapaths] = useState(initial.datapaths); // manual picks when there is no installer
  const [preview, setPreview] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);

  // The core decides everything; this only passes the current answers along.
  useEffect(() => {
    let current = true;
    loadPreview({ choices, datapaths }).then(
      (p) => current && setPreview(p),
      (e) => current && setError(String(e)),
    );
    return () => {
      current = false;
    };
    // loadPreview is rebuilt on every render of the parent; the answers are what matter.
  }, [choices, datapaths]);

  // Answers saved from the command line may leave the step out ("Variant=B"); a new
  // answer for that group replaces them, so the group never has two answers.
  function setGroup(step, group, options) {
    setChoices((all) => [
      ...all.filter((c) => !(c.group === group && (c.step === step || c.step === null))),
      ...options.map((option) => ({ step, group, option })),
    ]);
  }

  function discard() {
    setChoices(initial.choices);
    setDatapaths(initial.datapaths);
  }

  async function save() {
    setError(null);
    setBusy(true);
    try {
      await onSave(preview.manifest);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  // Takes the player to a question that still needs an answer.
  function goTo(step, group) {
    const el = document.getElementById(questionId(step, group));
    if (!el) return;
    el.scrollIntoView?.({ block: "center", behavior: "smooth" });
    el.querySelector("input:not(:disabled)")?.focus({ preventScroll: true });
  }

  const needsPicker = !fomod && candidates.length > 1;
  const decided = (preview?.manifest.decisions.datapaths.length ?? 0) > 0;
  const changed = Boolean(preview?.changed);
  const open = preview?.unanswered ?? [];
  const canSave = decided && open.length === 0 && (newMod || changed) && !busy;

  return (
    <section aria-label={title}>
      <header className="mod-header">
        <h2>{title}</h2>
        <p className="muted path" data-testid="folder-path">
          Folder: {folder}
          {folderMissing ? " (not found)" : ""}
        </p>
        {actions?.({ changed })}
      </header>

      {error && <p className="note error" role="alert">{error}</p>}
      {children}

      <div role="tablist" aria-label="About this mod" className="tabs">
        {TABS.map((t) => (
          <button
            key={t.id}
            role="tab"
            id={`tab-${t.id}`}
            aria-selected={tab === t.id}
            aria-controls={`panel-${t.id}`}
            onClick={() => setTab(t.id)}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div role="tabpanel" id={`panel-${tab}`} aria-labelledby={`tab-${tab}`}>
        {!preview ? (
          <p className="muted">Loading…</p>
        ) : tab === "setup" ? (
          <>
            {newMod && (
              <p>
                Found {candidates.length === 1 ? "one place" : `${candidates.length} places`} that could be the mod,
                with {candidates.reduce((n, c) => n + c.plugins.length, 0)} plugin(s).
              </p>
            )}
            {warnings.map((w, i) => (
              <p key={i} className="note" role="note">{describeWarning(w)}</p>
            ))}
            {fomod?.module_image && (
              <ModImage
                root={folder}
                path={fomod.module_image}
                alt={`${fomod.module_name ?? title}: banner from the author`}
                className="module-image"
              />
            )}
            {fomod && preview.fomod && (
              <FomodWizard
                info={fomod}
                resolution={preview.fomod}
                onChange={setGroup}
                root={folder}
                unanswered={preview.unanswered}
              />
            )}
            {needsPicker && <DatapathPicker candidates={candidates} chosen={datapaths} onChoose={setDatapaths} />}
            <Outcome preview={preview} onGoTo={goTo} />

            <div className="actions">
              <button onClick={save} disabled={!canSave}>
                {busy ? "Saving…" : saveLabel}
              </button>
              {!newMod && changed && (
                <button className="secondary" onClick={discard} disabled={busy}>
                  Discard
                </button>
              )}
            </div>
            {open.length > 0 ? (
              <p className="muted" role="status">
                {open.length === 1 ? "1 question" : `${open.length} questions`} to answer before this can be{" "}
                {newMod ? "added" : "saved"}: {open.map((q) => q.group).join(", ")}.
              </p>
            ) : (
              newMod && !decided && <p className="muted">Answer the questions above to add this mod.</p>
            )}
            {!newMod && changed && <p className="muted">These answers are not saved yet.</p>}
          </>
        ) : tab === "notes" ? (
          <AuthorNotes notes={preview.manifest.derived.notes} />
        ) : tab === "needs" ? (
          <Requirements requirements={preview.requirements} />
        ) : (
          unknown("tab", tab)
        )}
      </div>
    </section>
  );
}
