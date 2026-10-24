import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import missing from "../../test-data/preview-simple-missing.json";
import satisfied from "../../test-data/preview-simple-satisfied.json";
import unknownAvail from "../../test-data/preview-simple-unknown.json";
import Requirements from "./Requirements.jsx";

describe("Requirements (rendering real core output)", () => {
  it("says a master is ready when it is available", () => {
    render(<Requirements requirements={satisfied.requirements} />);
    expect(screen.getByText("Morrowind.esm")).toBeInTheDocument();
    expect(screen.getByText("ready")).toBeInTheDocument();
    expect(screen.getByText("for Simple.esp")).toBeInTheDocument();
  });

  it("says a master is missing in words, not only colour", () => {
    render(<Requirements requirements={missing.requirements} />);
    expect(screen.getByText("missing")).toBeInTheDocument();
  });

  it("says when it cannot tell", () => {
    render(<Requirements requirements={unknownAvail.requirements} />);
    expect(screen.getByText("can't tell")).toBeInTheDocument();
  });

  it("says so when there is nothing to need", () => {
    render(<Requirements requirements={[]} />);
    expect(screen.getByText("Nothing else.")).toBeInTheDocument();
  });

  it("refuses to guess at an unknown status", () => {
    const bad = [{ ...satisfied.requirements[0], status: "kinda" }];
    const quiet = () => {};
    const spy = console.error;
    console.error = quiet; // React logs the thrown error; the throw is what we assert
    try {
      expect(() => render(<Requirements requirements={bad} />)).toThrow(/requirement status/);
    } finally {
      console.error = spy;
    }
  });
});
