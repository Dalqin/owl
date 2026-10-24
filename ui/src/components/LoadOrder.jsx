import { describeIssue, describeSource, issuePlugin } from "../describe.js";

// Every plugin in the order OpenMW will load it, with anything wrong beside it.
export default function LoadOrder({ order, issues }) {
  return (
    <section aria-labelledby="order-heading">
      <h3 id="order-heading">Load order</h3>
      <p className="muted">Earlier loads first. A plugin that needs another must come after it.</p>
      <table className="order">
        <thead>
          <tr>
            <th scope="col">#</th>
            <th scope="col">Plugin</th>
            <th scope="col">From</th>
            <th scope="col">
              Problems
              <br />
              <span className="muted th-sub">(if any)</span>
            </th>
          </tr>
        </thead>
        <tbody>
          {order.map((entry, i) => {
            const notes = issues
              .filter((issue) => issuePlugin(issue)?.toLowerCase() === entry.plugin.toLowerCase())
              .map(describeIssue);
            return (
              <tr key={`${i}-${entry.plugin}`} className={notes.length ? "has-note" : undefined}>
                <td>{i + 1}</td>
                <td>{entry.plugin}</td>
                <td className="muted">{describeSource(entry.source)}</td>
                <td>{notes.join(" ")}</td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </section>
  );
}
