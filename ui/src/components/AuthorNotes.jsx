import { unknown } from "../unknown.js";

// What one document looks like to the player. `text` is plain text made by the
// core (HTML is already flattened there), so it is shown as text and nothing else.
function Document({ doc, open }) {
  let body;
  switch (doc.kind) {
    case "text":
    case "markdown":
    case "html":
    case "link":
      body = doc.text === null ? null : <pre className="author-text">{doc.text}</pre>;
      break;
    case "other":
      body = <p className="muted">This file is not shown here. Open it from the mod's folder.</p>;
      break;
    default:
      unknown("document kind", doc.kind);
  }
  return (
    <details open={open}>
      <summary className="path">{doc.path}</summary>
      {doc.error && <p className="note" role="note">Could not be read: {doc.error}</p>}
      {body}
      {doc.truncated && (
        <p className="muted">Only the first part is shown. Open the file in the mod's folder for the rest.</p>
      )}
    </details>
  );
}

// Everything the mod's author left for the people installing it: the installer's
// own information, and the readme-like files in the mod's folder.
export default function AuthorNotes({ notes }) {
  const { info, documents } = notes;
  if (!info && documents.length === 0) {
    return (
      <section aria-labelledby="notes-heading">
        <h3 id="notes-heading">From the mod's author</h3>
        <p className="muted">The author left no notes in this folder.</p>
      </section>
    );
  }

  const facts = info
    ? [
        ["Name", info.name],
        ["Author", info.author],
        ["Version", info.version],
        ["Website", info.website],
        ...info.other.map((f) => [f.name, f.value]),
      ].filter(([, value]) => value)
    : [];

  return (
    <section aria-labelledby="notes-heading">
      <h3 id="notes-heading">From the mod's author</h3>
      {facts.length > 0 && (
        <dl className="outcome">
          {facts.map(([name, value], i) => (
            <div key={`${i}-${name}`} className="fact">
              <dt>{name}</dt>
              <dd>{value}</dd>
            </div>
          ))}
        </dl>
      )}
      {info?.description && (
        <>
          <h4>Description</h4>
          <pre className="author-text">{info.description}</pre>
        </>
      )}
      {documents.length > 0 && (
        <>
          <h4>Files from the author</h4>
          {documents.map((doc, i) => (
            <Document key={`${i}-${doc.path}`} doc={doc} open={documents.length === 1} />
          ))}
        </>
      )}
    </section>
  );
}
