import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import changed from "../test-data/apply-plan-changed.json";
import needed from "../test-data/order-fix-needed.json";
import ApplyPanel from "./components/ApplyPanel.jsx";
import OrderFixPanel from "./components/OrderFixPanel.jsx";
import Outcome from "./components/Outcome.jsx";
import preview from "../test-data/preview-fomod-default.json";

vi.mock("./api.js", () => ({ planApply: vi.fn(), planOrderFix: vi.fn(), fixOrder: vi.fn(), applyChanges: vi.fn() }));
import * as api from "./api.js";

// Lists built from text that can legitimately repeat must not use the text as a React key.
let errors;
beforeEach(() => {
  vi.resetAllMocks();
  errors = vi.spyOn(console, "error").mockImplementation(() => {});
});
afterEach(() => errors.mockRestore());

const keyComplaints = () => errors.mock.calls.filter((c) => String(c[0]).includes("same key"));

describe("repeated text in lists", () => {
  it("the apply review copes with a comment banner that repeats a line", async () => {
    // What the OpenMW Launcher leaves in openmw.cfg: a boxed banner whose top and bottom lines are identical.
    const banner = ["", "#               #", "# CONTENT FILES #", "#               #", "content=Morrowind.esm"];
    api.planApply.mockResolvedValue({ ...changed, removed: banner,
      added: ["x", "x"],
      diff: [
        ...banner.map((text, i) => ({ kind: "removed", old_line: i + 1, new_line: null, text })),
        ...["x", "x"].map((text, i) => ({ kind: "added", old_line: null, new_line: i + 1, text })),
      ],
    });
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    const diff = await screen.findByLabelText("Changes");
    expect(keyComplaints()).toEqual([]);
    expect(diff.textContent.match(/#               #/g)).toHaveLength(2); // both lines are shown, none dropped
    expect(diff.textContent.match(/\+ x/g)).toHaveLength(2);
  });

  it("the order fix copes with two mods that share a name", async () => {
    api.planOrderFix.mockResolvedValue({
      ...needed,
      order: ["Same", "Same", "Other"],
      moves: [
        { name: "Same", from: 2, to: 0 },
        { name: "Same", from: 3, to: 1 },
      ],
    });
    render(<OrderFixPanel cfgPath="/cfg" onFixed={() => {}} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    await screen.findByLabelText("New order");
    expect(keyComplaints()).toEqual([]);
    expect(screen.getByLabelText("New order").children).toHaveLength(3);
  });

  it("repeated notes are all shown", () => {
    const dup = { ...preview, fomod: { ...preview.fomod, problems: [
      { kind: "source_not_found", source: "X" },
      { kind: "source_not_found", source: "X" },
    ] } };
    render(<Outcome preview={dup} />);
    expect(keyComplaints()).toEqual([]);
    expect(screen.getAllByRole("note")).toHaveLength(2);
  });
});
