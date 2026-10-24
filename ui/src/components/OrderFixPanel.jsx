import { useState } from "react";

import * as api from "../api.js";

// Offers to reorder the player's list so every plugin loads after the plugins
// it needs. Shows what will move first; changes nothing until confirmed.
export default function OrderFixPanel({ cfgPath, onFixed }) {
  const [fix, setFix] = useState(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState(null);

  async function review() {
    setError(null);
    setBusy(true);
    try {
      setFix(await api.planOrderFix(cfgPath));
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function confirm() {
    setError(null);
    setBusy(true);
    try {
      const done = await api.fixOrder(cfgPath);
      setFix(null);
      onFixed(done);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const movedUp = fix?.moves.filter((m) => m.to < m.from) ?? [];

  return (
    <section aria-labelledby="fix-heading" className="fix">
      <h3 id="fix-heading">Fix the order</h3>
      {error && <p className="note error" role="alert">{error}</p>}

      {!fix && (
        <>
          <p>Some mods load before a mod they need. Reordering your list can fix that.</p>
          <button onClick={review} disabled={busy}>{busy ? "Checking…" : "Fix the order for me"}</button>
        </>
      )}

      {fix && fix.cyclic_mods.length > 0 && (
        <>
          <p className="note">
            These mods need each other in a loop, so no order of whole mods works: {fix.cyclic_mods.join(", ")}.
            Remove one of them, or use a version that does not depend on the other.
          </p>
          <button className="secondary" onClick={() => setFix(null)}>Close</button>
        </>
      )}

      {fix && fix.cyclic_mods.length === 0 && !fix.changed && (
        <>
          <p>Moving mods around would not fix the remaining problems.</p>
          <button className="secondary" onClick={() => setFix(null)}>Close</button>
        </>
      )}

      {fix && fix.changed && (
        <>
          <p>A mod that others need moves up to just before the first one that needs it. Nothing else is moved.</p>
          <ul className="fix-moves">
            {movedUp.map((m) => (
              <li key={`${m.from}-${m.name}`}>
                <strong>{m.name}</strong> moves up, from position {m.from + 1} to {m.to + 1}.
              </li>
            ))}
            {fix.plugin_reorders.map((r, i) => (
              <li key={`${i}-${r.mod_name}`}>
                <strong>{r.mod_name}</strong>: its plugins load in this order: {r.after.join(", ")}.
              </li>
            ))}
          </ul>
          <p className="muted">New order:</p>
          <ol className="fix-order" aria-label="New order">
            {fix.order.map((name, i) => (
              <li key={`${i}-${name}`}>{name}</li>
            ))}
          </ol>
          <p className="muted">
            This only changes the order of your list. Your mods' files are not touched, and OpenMW's settings stay as
            they are until you review and write them below.
          </p>
          <div className="actions">
            <button onClick={confirm} disabled={busy}>Reorder my mods</button>
            <button className="secondary" onClick={() => setFix(null)} disabled={busy}>Cancel</button>
          </div>
        </>
      )}
    </section>
  );
}
