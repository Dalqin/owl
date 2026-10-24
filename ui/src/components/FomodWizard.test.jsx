import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import defaults from "../../test-data/preview-fomod-default.json";
import chosenB from "../../test-data/preview-fomod-b.json";
import scan from "../../test-data/scan-fomod.json";
import FomodWizard from "./FomodWizard.jsx";

describe("FomodWizard (rendering real core output)", () => {
  it("shows the installer's question and options", () => {
    render(<FomodWizard info={scan.fomod} resolution={defaults.fomod} onChange={() => {}} />);
    expect(screen.getByText("Installer: Fixture FOMOD")).toBeInTheDocument();
    expect(screen.getByText("Pick a variant")).toBeInTheDocument();
    expect(screen.getByLabelText(/^A/)).toBeChecked(); // the installer's default
    expect(screen.getByLabelText(/^B/)).not.toBeChecked();
    expect(screen.getByText("The first variant")).toBeInTheDocument();
    expect(screen.getByText("recommended")).toBeInTheDocument();
  });

  it("reflects the answer the core resolved", () => {
    render(<FomodWizard info={scan.fomod} resolution={chosenB.fomod} onChange={() => {}} />);
    expect(screen.getByLabelText(/^B/)).toBeChecked();
    expect(screen.getByLabelText(/^A/)).not.toBeChecked();
  });

  it("reports the full new selection for the group when an option is picked", () => {
    const onChange = vi.fn();
    render(<FomodWizard info={scan.fomod} resolution={defaults.fomod} onChange={onChange} />);
    fireEvent.click(screen.getByLabelText(/^B/));
    expect(onChange).toHaveBeenCalledWith("Pick a variant", "Variant", ["B"]);
  });

  it("hides steps the core says are not shown", () => {
    const resolution = { ...defaults.fomod, visible_steps: [] };
    render(<FomodWizard info={scan.fomod} resolution={resolution} onChange={() => {}} />);
    expect(screen.queryByText("Pick a variant")).not.toBeInTheDocument();
  });

  it("refuses to guess at an unknown group kind", () => {
    const info = structuredClone(scan.fomod);
    info.steps[0].groups[0].kind = "select_some";
    const spy = console.error;
    console.error = () => {};
    try {
      expect(() => render(<FomodWizard info={info} resolution={defaults.fomod} onChange={() => {}} />)).toThrow(/group kind/);
    } finally {
      console.error = spy;
    }
  });

  it("reading more about an option does not select it", () => {
    const onChange = vi.fn();
    render(<FomodWizard info={scan.fomod} resolution={defaults.fomod} onChange={onChange} />);
    fireEvent.click(screen.getByRole("button", { name: "Read more about B" }));
    expect(screen.getByText(/Use this one if you also use the Example mod/)).toBeInTheDocument();
    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByLabelText(/^B/)).not.toBeChecked();
  });

  it("each option's name still selects it", () => {
    const onChange = vi.fn();
    render(<FomodWizard info={scan.fomod} resolution={defaults.fomod} onChange={onChange} />);
    fireEvent.click(screen.getByText("B"));
    expect(onChange).toHaveBeenCalledWith("Pick a variant", "Variant", ["B"]);
  });
});
