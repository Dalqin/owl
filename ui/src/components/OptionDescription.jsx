import { useId, useState } from "react";

// First meaningful line of the author's text (skips lines that are only decoration like "*****").
function firstLine(text) {
  return text.split("\n").map((l) => l.trim()).find((l) => /\p{L}/u.test(l)) ?? "";
}

// The mod author's description of an option. Shows its first line; if there is
// more, a button reveals all of it exactly as the author wrote it.
export default function OptionDescription({ name, text }) {
  const [open, setOpen] = useState(false);
  const id = useId();

  const full = text.replace(/\r\n?/g, "\n").replace(/\n{3,}/g, "\n\n").trim();
  const first = firstLine(full);
  if (!first) return null;
  const hasMore = full !== first;

  return (
    <span className="option-desc-wrap">
      {open ? (
        <span id={id} className="muted option-desc option-desc-full">{full}</span>
      ) : (
        <span id={id} className="muted option-desc">{first}</span>
      )}
      {hasMore && (
        <button
          type="button"
          className="link-button"
          aria-expanded={open}
          aria-controls={id}
          aria-label={`${open ? "Show less" : "Read more"} about ${name}`}
          onClick={() => setOpen(!open)}
        >
          {open ? "Show less" : "Read more"}
        </button>
      )}
    </span>
  );
}
