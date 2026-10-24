import { describeRequirement, statusLabel } from "../describe.js";

// "A mod needs these things" - informative (what) and helpful (is it covered).
export default function Requirements({ requirements }) {
  return (
    <section aria-labelledby="needs-heading">
      <h3 id="needs-heading">This mod needs these things</h3>
      {requirements.length === 0 ? (
        <p className="muted">Nothing else.</p>
      ) : (
        <ul className="needs">
          {requirements.map((r) => {
            const label = describeRequirement(r);
            const status = r.status;
            return (
              <li key={label} className={`need need-${status}`}>
                <span className="need-name">{label}</span>
                <span className="need-status">{statusLabel(status)}</span>
                <span className="muted need-by">for {r.needed_by.join(", ")}</span>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
