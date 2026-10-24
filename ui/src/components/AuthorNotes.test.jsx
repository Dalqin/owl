import { render, screen, within } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import notesView from "../../test-data/profile-view-notes.json";
import healthy from "../../test-data/profile-view-healthy.json";
import AuthorNotes from "./AuthorNotes.jsx";

const notes = notesView.mods[0].manifest.derived.notes;

describe("AuthorNotes (rendering real core output)", () => {
  it("shows what the installer's info file says about the mod", () => {
    render(<AuthorNotes notes={notes} />);
    const facts = screen.getByText("Author", { selector: "dt" }).closest("div");
    expect(within(facts).getByText("An Author")).toBeInTheDocument();
    expect(screen.getByText("1.2.3")).toBeInTheDocument();
    expect(screen.getByText("https://example.org/mods/notes", { selector: "dd" })).toBeInTheDocument();
    expect(screen.getByText("Id", { selector: "dt" }).closest("div")).toHaveTextContent("12345");
  });

  it("shows the author's whole description with their line breaks", () => {
    render(<AuthorNotes notes={notes} />);
    const text = screen.getByText(/Requirements:/);
    expect(text.textContent).toBe(notes.info.description);
    expect(text.textContent).toContain("Morrowind & Tribunal");
    expect(text.textContent).toContain("Known issues:\nNone.");
  });

  it("lists every file the author left, with its text", () => {
    render(<AuthorNotes notes={notes} />);
    const paths = screen.getAllByText(/README\.txt|Install\.html|Link\.url/).map((e) => e.textContent);
    expect(paths).toEqual(["Link.url", "README.txt", "Docs/Install.html"]);
    expect(screen.getByText(/Café patch for “Morrowind” – read this first\./)).toBeInTheDocument();
    expect(screen.getByText("https://example.org/mods/notes", { selector: "pre" })).toBeInTheDocument();
  });

  it("says plainly when the author left nothing", () => {
    render(<AuthorNotes notes={healthy.mods[0].manifest.derived.notes} />);
    expect(screen.getByText("The author left no notes in this folder.")).toBeInTheDocument();
  });

  it("says when only the start of a long file is shown, and when a file could not be read", () => {
    const doc = (extra) => ({ path: "readme.txt", kind: "text", text: "start", truncated: false, error: null, ...extra });
    const { unmount } = render(<AuthorNotes notes={{ info: null, documents: [doc({ truncated: true })] }} />);
    expect(screen.getByText(/Only the first part is shown/)).toBeInTheDocument();
    unmount();
    render(<AuthorNotes notes={{ info: null, documents: [doc({ text: null, error: "permission denied" })] }} />);
    expect(screen.getByRole("note")).toHaveTextContent("Could not be read: permission denied");
  });

  it("lists a file it cannot show as text, without pretending to show it", () => {
    const pdf = { path: "Manual.pdf", kind: "other", text: null, truncated: false, error: null };
    render(<AuthorNotes notes={{ info: null, documents: [pdf] }} />);
    expect(screen.getByText("Manual.pdf")).toBeInTheDocument();
    expect(screen.getByText(/not shown here/)).toBeInTheDocument();
  });

  it("refuses to guess at a kind of file it does not know", () => {
    const odd = { path: "x", kind: "hologram", text: null, truncated: false, error: null };
    const quiet = () => {};
    const original = console.error;
    console.error = quiet;
    try {
      expect(() => render(<AuthorNotes notes={{ info: null, documents: [odd] }} />)).toThrow(/document kind/);
    } finally {
      console.error = original;
    }
  });
});
