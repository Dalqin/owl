import { useState } from "react";

import { describeIssue } from "../describe.js";
import ApplyPanel from "./ApplyPanel.jsx";
import LoadOrder from "./LoadOrder.jsx";
import OrderFixPanel from "./OrderFixPanel.jsx";

// What is true of the list as a whole: whether it holds together, the order
// plugins will load in, and writing it into OpenMW's settings.
export default function AllModules({ view, cfgPath, profileFile, onReload, onAdd }) {
  const [notice, setNotice] = useState(null);
  const { mods, check } = view;

  if (mods.length === 0) {
    return (
      <section aria-label="Review and finalize">
        <p>You have not added any mods yet.</p>
        <button onClick={onAdd}>Manage my sources</button>
      </section>
    );
  }

  return (
    <section aria-label="Review and finalize">
      {notice && <p className="ok" role="status">{notice}</p>}

      <h2>Review and finalize</h2>
      {check.issues.length === 0 ? (
        <p className="ok" role="status">No problems found.</p>
      ) : (
        <div role="status">
          <p className="problem-count">
            {check.issues.length === 1 ? "1 problem" : `${check.issues.length} problems`} to look at:
          </p>
          {check.issues.map((issue, i) => (
            <p key={i} className="note">{describeIssue(issue)}</p>
          ))}
        </div>
      )}

      {check.issues.some((i) => i.kind === "master_after_plugin") && (
        <OrderFixPanel
          cfgPath={cfgPath}
          onFixed={(fix) => {
            setNotice(
              fix.changed
                ? "Your list is reordered. Review the changes below to update OpenMW."
                : "The order was already fine.",
            );
            onReload();
          }}
        />
      )}

      <LoadOrder order={check.load_order} issues={check.issues} />

      <ApplyPanel canApply={Boolean(cfgPath)} />
      {profileFile && <p className="muted path">Your list is saved at {profileFile}</p>}
    </section>
  );
}
