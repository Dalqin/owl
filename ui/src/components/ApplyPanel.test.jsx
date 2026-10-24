import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import blocked from "../../test-data/apply-plan-blocked.json";
import changed from "../../test-data/apply-plan-changed.json";
import kept from "../../test-data/apply-plan-kept.json";
import unchanged from "../../test-data/apply-plan-unchanged.json";
import written from "../../test-data/apply-outcome-written.json";
import ApplyPanel from "./ApplyPanel.jsx";

vi.mock("../api.js", () => ({ planApply: vi.fn(), applyChanges: vi.fn() }));
import * as api from "../api.js";

beforeEach(() => vi.resetAllMocks());

describe("ApplyPanel (rendering real core output)", () => {
  it("changes nothing until the player has reviewed and confirmed", async () => {
    api.planApply.mockResolvedValue(changed);
    render(<ApplyPanel canApply />);
    expect(api.applyChanges).not.toHaveBeenCalled();
    fireEvent.click(screen.getByText("Review changes"));
    await screen.findByText("Write to OpenMW settings");
    expect(api.applyChanges).not.toHaveBeenCalled();
  });

  it("shows exactly which lines will be added, and where the file is", async () => {
    api.planApply.mockResolvedValue(changed);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    const diff = await screen.findByLabelText("Changes");
    expect(diff).toHaveTextContent('+ data="/fixtures/simple"');
    expect(diff).toHaveTextContent("+ content=Simple.esp");
    expect(diff).toHaveTextContent("+ content=Variant.ESP");
    expect(screen.getByText(changed.cfg_path)).toBeInTheDocument();
    expect(screen.getByText(/saved next to it as a backup/)).toBeInTheDocument();
  });

  it("writes only what was reviewed, then says where the backup is", async () => {
    api.planApply.mockResolvedValue(changed);
    api.applyChanges.mockResolvedValue(written);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    fireEvent.click(await screen.findByText("Write to OpenMW settings"));
    await waitFor(() => expect(api.applyChanges).toHaveBeenCalledWith(changed.fingerprint, false));
    expect(await screen.findByText("Done. OpenMW's settings are updated.")).toBeInTheDocument();
    expect(screen.getByText(/openmw\.cfg\.bak/)).toBeInTheDocument();
  });

  it("says which lines in Owl's section it leaves alone, and removes only its own", async () => {
    api.planApply.mockResolvedValue(kept);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    const diff = await screen.findByLabelText("Changes");
    expect(diff).toHaveTextContent("− content=Variant.ESP");
    // It is shown as unchanged context, never as something removed.
    const row = [...diff.querySelectorAll(".row")].find((r) => r.textContent.includes("Bloodmoon.esm"));
    expect(row).toHaveClass("context");
    expect(diff).not.toHaveTextContent("− content=Bloodmoon.esm");
    expect(screen.getByText(/left exactly where they are: content=Bloodmoon\.esm/)).toBeInTheDocument();
  });

  it("shows unchanged lines around the changes, with line numbers in the old and new file", async () => {
    api.planApply.mockResolvedValue(changed);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    const rows = (await screen.findByLabelText("Changes")).querySelectorAll(".row");
    const added = [...rows].find((r) => r.classList.contains("added"));
    expect(rows.length).toBeGreaterThan(changed.added.length);
    expect([...rows].some((r) => r.classList.contains("context"))).toBe(true);
    const [oldNo, newNo] = added.querySelectorAll(".ln");
    expect(oldNo).toHaveTextContent("");
    expect(newNo.textContent).toMatch(/^\d+$/);
  });

  it("says so, and offers no write, when nothing would change", async () => {
    api.planApply.mockResolvedValue(unchanged);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    expect(await screen.findByText(/Nothing to change/)).toBeInTheDocument();
    expect(screen.queryByText("Write to OpenMW settings")).not.toBeInTheDocument();
  });

  it("shows problems first and does not offer the normal write", async () => {
    api.planApply.mockResolvedValue(blocked);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    expect(await screen.findByText("Fix these first:")).toBeInTheDocument();
    expect(screen.getByText(/needs Tribunal\.esm, which is not loaded/)).toBeInTheDocument();
    expect(screen.queryByText("Write to OpenMW settings")).not.toBeInTheDocument();
  });

  it("only forces a write when the player chooses 'Write anyway'", async () => {
    api.planApply.mockResolvedValue(blocked);
    api.applyChanges.mockResolvedValue(written);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    fireEvent.click(await screen.findByText("Write anyway"));
    await waitFor(() => expect(api.applyChanges).toHaveBeenCalledWith(blocked.fingerprint, true));
  });

  it("cancelling writes nothing", async () => {
    api.planApply.mockResolvedValue(changed);
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    fireEvent.click(await screen.findByText("Cancel"));
    expect(screen.getByText("Review changes")).toBeInTheDocument();
    expect(api.applyChanges).not.toHaveBeenCalled();
  });

  it("if the file changed since the review, shows the error and starts over", async () => {
    api.planApply.mockResolvedValue(changed);
    api.applyChanges.mockRejectedValue("your OpenMW settings changed since you reviewed them; review again");
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    fireEvent.click(await screen.findByText("Write to OpenMW settings"));
    expect(await screen.findByRole("alert")).toHaveTextContent("changed since you reviewed them");
    expect(screen.getByText("Review changes")).toBeInTheDocument();
    expect(screen.queryByText("Write to OpenMW settings")).not.toBeInTheDocument();
  });

  it("explains instead of offering a button when OpenMW's settings were not found", () => {
    render(<ApplyPanel canApply={false} />);
    expect(screen.getByText(/Could not find your OpenMW settings/)).toBeInTheDocument();
    expect(screen.queryByText("Review changes")).not.toBeInTheDocument();
  });

  it("shows an error from the review step", async () => {
    api.planApply.mockRejectedValue("managed-region markers are unbalanced");
    render(<ApplyPanel canApply />);
    fireEvent.click(screen.getByText("Review changes"));
    expect(await screen.findByRole("alert")).toHaveTextContent("unbalanced");
  });
});
