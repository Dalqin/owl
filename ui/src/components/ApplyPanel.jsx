import { useState } from "react";

import * as api from "../api.js";
import { describeIssue } from "../describe.js";

const SIGN = { context: " ", added: "+", removed: "−" };

// Review, then confirm: nothing is written until the player has seen exactly
// which lines will change.
export default function ApplyPanel({ canApply }) {
  const [plan, setPlan] = useState(null);
  const [outcome, setOutcome] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);

  async function review() {
    setError(null);
    setOutcome(null);
    setBusy(true);
    try {
      setPlan(await api.planApply());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function write(force) {
    setError(null);
    setBusy(true);
    try {
      setOutcome(await api.applyChanges(plan.fingerprint, force));
      setPlan(null);
    } catch (e) {
      // Includes "changed since you reviewed them": start over from a fresh review.
      setError(String(e));
      setPlan(null);
    } finally {
      setBusy(false);
    }
  }

  return (
    <section aria-labelledby="apply-heading">
      <h3 id="apply-heading">Use these mods in OpenMW</h3>

      {error && <p className="note error" role="alert">{error}</p>}

      {!canApply && <p className="muted">Could not find your OpenMW settings, so there is nothing to update.</p>}

      {canApply && !plan && !outcome && (
        <>
          <p>See exactly what would change in OpenMW's settings before anything is written.</p>
          <button onClick={review} disabled={busy}>{busy ? "Checking…" : "Review changes"}</button>
        </>
      )}

      {outcome && (
        <div role="status">
          {outcome.changed ? (
            <>
              <p className="ok">Done. OpenMW's settings are updated.</p>
              {outcome.backup && <p className="muted path">The previous version is saved at {outcome.backup}</p>}
            </>
          ) : (
            <p className="ok">OpenMW's settings were already up to date.</p>
          )}
          <p>You can start OpenMW now.</p>
        </div>
      )}

      {plan && (
        <div>
          <p className="muted path">{plan.cfg_path}</p>

          {plan.issues.length > 0 && (
            <div role="alert">
              <p className="problem-count">Fix these first:</p>
              {plan.issues.map((issue, i) => (
                <p key={i} className="note">{describeIssue(issue)}</p>
              ))}
            </div>
          )}

          {!plan.changed ? (
            <p>Nothing to change. OpenMW's settings are already up to date.</p>
          ) : (
            <>
              <p>This will change only Owl's section of the file. Your own lines are left exactly as they are. Numbers on the left are the line in the current file, then in the new one.</p>
              <pre className="diff" aria-label="Changes">
                {/* Lines can repeat (a comment banner has the same line twice), so the position is part of the key. */}
                {plan.diff.map((line, i) =>
                  line.kind === "gap" ? (
                    <span key={i} className="row gap" aria-hidden="true">{"⋯\n"}</span>
                  ) : (
                    <span key={i} className={`row ${line.kind}`}>
                      <span className="ln" aria-hidden="true">{line.old_line ?? ""}</span>
                      <span className="ln" aria-hidden="true">{line.new_line ?? ""}</span>
                      {`${SIGN[line.kind]} ${line.text}\n`}
                    </span>
                  ),
                )}
              </pre>
              <p className="muted">The current file is saved next to it as a backup (.bak) first.</p>
            </>
          )}

          {plan.kept.length > 0 && (
            <p className="muted">
              Also in Owl's section but not added by Owl (OpenMW's launcher puts the plugins you tick there), so left exactly where they are: {plan.kept.join(", ")}
            </p>
          )}

          {plan.skipped_duplicates.length > 0 && (
            <p className="muted">
              Already in your settings, so left alone: {plan.skipped_duplicates.join(", ")}
            </p>
          )}

          <div className="actions">
            {plan.changed && plan.issues.length === 0 && (
              <button onClick={() => write(false)} disabled={busy}>Write to OpenMW settings</button>
            )}
            {plan.changed && plan.issues.length > 0 && (
              <button className="secondary" onClick={() => write(true)} disabled={busy}>
                Write anyway
              </button>
            )}
            <button className="secondary" onClick={() => setPlan(null)} disabled={busy}>
              {plan.changed ? "Cancel" : "Close"}
            </button>
          </div>
        </div>
      )}
    </section>
  );
}
