import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import libraryView from "../../test-data/profile-view-library.json";
import healthy from "../../test-data/profile-view-healthy.json";
import problems from "../../test-data/profile-view-problems.json";
import Sidebar from "./Sidebar.jsx";

const show = (view, selection, onSelect = () => {}, extra = {}) =>
  render(
    <Sidebar
      mods={view.mods}
      check={view.check}
      libraries={view.libraries}
      individuals={view.individuals}
      selection={selection}
      onSelect={onSelect}
      {...extra}
    />,
  );

describe("Sidebar", () => {
  it("pins adding a mod above the mods, and all modules below them", () => {
    show(healthy, { kind: "list" });
    const buttons = screen.getAllByRole("button").map((b) => b.textContent);
    expect(buttons).toEqual(["+ Add a mod", "1.Simple", "2.Variant", "Review and finalize", "Help"]);
  });

  it("marks only the selected entry as current", () => {
    show(healthy, { kind: "mod", index: 1 });
    const current = screen.getAllByRole("button").filter((b) => b.getAttribute("aria-current") === "page");
    expect(current.map((b) => b.textContent)).toEqual(["2.Variant"]);
  });

  it("reports what the player picked", () => {
    const onSelect = vi.fn();
    show(healthy, { kind: "list" }, onSelect);
    fireEvent.click(screen.getByRole("button", { name: /Simple/ }));
    fireEvent.click(screen.getByRole("button", { name: "+ Add a mod" }));
    fireEvent.click(screen.getByRole("button", { name: "Review and finalize" }));
    fireEvent.click(screen.getByRole("button", { name: "Help" }));
    expect(onSelect.mock.calls.map((c) => c[0])).toEqual([
      { kind: "mod", index: 0 },
      { kind: "add", pick: expect.any(Number) }, // asks for the folder chooser
      { kind: "list" },
      { kind: "help" },
    ]);
  });

  it("flags the mods that have problems, and a missing folder, and counts them for all modules", () => {
    show(problems, { kind: "list" });
    const flagged = screen.getAllByRole("listitem").map((li) => within(li).queryByRole("img", { name: "has problems" }) !== null);
    expect(flagged).toEqual([true, true, true]);
    expect(screen.getByRole("img", { name: "3 problems" })).toBeInTheDocument();
  });

  it("shows no flags when everything is fine", () => {
    show(healthy, { kind: "list" });
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("lists the libraries above the mods, and the review last, with only what can still be added", () => {
    show(libraryView, { kind: "list" });
    const buttons = screen.getAllByRole("button").map((b) => b.textContent);
    expect(buttons).toEqual([
      "+ Add a mod",
      "simple", // individual mods come first
      "✕",
      "no-such-modmissing",
      "✕",
      "library1",
      "✕",
      "Beta Installer",
      "no-such-librarymissing",
      "✕",
      "1.Alpha Mod",
      "Review and finalize",
      "Help",
    ]);
  });

  it("groups single mods under a heading that says they are not from a folder", () => {
    show(libraryView, { kind: "list" });
    expect(screen.getByText("Individual mods")).toBeInTheDocument();
    expect(screen.getByText("Not part of a folder")).toBeInTheDocument();
  });

  it("has no individual group when there are none", () => {
    show(healthy, { kind: "list" });
    expect(screen.queryByText("Individual mods")).not.toBeInTheDocument();
  });

  it("marks the open individual mod, and forgets one or a folder on request", () => {
    const onForgetIndividual = vi.fn();
    const onForgetLibrary = vi.fn();
    show(libraryView, { kind: "candidate", path: "/fixtures/simple" }, () => {}, { onForgetIndividual, onForgetLibrary });
    const current = screen.getAllByRole("button").filter((b) => b.getAttribute("aria-current") === "page");
    expect(current.map((b) => b.textContent)).toEqual(["simple"]);
    fireEvent.click(screen.getByRole("button", { name: "Forget simple" }));
    fireEvent.click(screen.getByRole("button", { name: "Stop listing library" }));
    expect(onForgetIndividual).toHaveBeenCalledWith("/fixtures/simple");
    expect(onForgetLibrary).toHaveBeenCalledWith("/fixtures/library");
  });

  it("marks the open library or candidate, and reports what the player picks in them", () => {
    const onSelect = vi.fn();
    const { rerender } = render(
      <Sidebar mods={libraryView.mods} check={libraryView.check} libraries={libraryView.libraries} selection={{ kind: "candidate", path: "/fixtures/library/Beta Installer" }} onSelect={onSelect} />,
    );
    const current = () => screen.getAllByRole("button").filter((b) => b.getAttribute("aria-current") === "page").map((b) => b.textContent);
    expect(current()).toEqual(["Beta Installer"]);
    rerender(
      <Sidebar mods={libraryView.mods} check={libraryView.check} libraries={libraryView.libraries} selection={{ kind: "library", path: "/fixtures/library" }} onSelect={onSelect} />,
    );
    expect(current()).toEqual(["library1"]);
    fireEvent.click(screen.getByRole("button", { name: "Beta Installer" }));
    fireEvent.click(screen.getByRole("button", { name: /^no-such-library/ }));
    expect(onSelect.mock.calls.map((c) => c[0])).toEqual([
      { kind: "candidate", path: "/fixtures/library/Beta Installer" },
      { kind: "library", path: "/fixtures/no-such-library" },
    ]);
  });

  it("shows no library section when there are none", () => {
    show(healthy, { kind: "list" });
    expect(screen.queryByText("Available to add")).not.toBeInTheDocument();
  });
});
