import { describeDecisionIssue, describeProblem } from "../describe.js";

// What will actually be set up, and anything the player should know about it.
//
// `onGoTo(step, group)` takes the player to an installer question that still needs an answer.
export default function Outcome({ preview, onGoTo }) {
  const { datapaths, enabled_plugins: plugins } = preview.manifest.decisions;
  // Questions still to answer are shown on their own, below; the rest are shown as notes.
  const problems = (preview.fomod?.problems ?? []).filter(
    (p) => !(p.kind === "group_unresolved" && p.cause === "choice_needed"),
  );
  const issues = preview.issues;
  return (
    <section aria-labelledby="outcome-heading">
      <h3 id="outcome-heading">What will be set up</h3>
      {datapaths.length === 0 ? (
        <p className="muted">Nothing yet. Answer the questions above.</p>
      ) : (
        <dl className="outcome">
          <dt>Folders</dt>
          <dd>{datapaths.map((p) => (p === "" ? "(this folder)" : p)).join(", ")}</dd>
          <dt>Plugins</dt>
          <dd>{plugins.length ? plugins.join(", ") : "none"}</dd>
        </dl>
      )}
      {[...problems.map(describeProblem), ...issues.map(describeDecisionIssue)].map((text, i) => (
        <p key={i} className="note" role="note">{text}</p>
      ))}
      {preview.unanswered.length > 0 && (
        <div className="note" role="note" aria-label="Still to answer">
          <p className="problem-count">Still to answer:</p>
          <ul className="open-questions">
            {preview.unanswered.map((q, i) => (
              <li key={`${i}-${q.step}-${q.group}`}>
                <span>
                  {q.step} → {q.group}
                </span>{" "}
                {onGoTo && (
                  <button type="button" className="link-button" onClick={() => onGoTo(q.step, q.group)}>
                    Go to this question
                  </button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  );
}
