import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import loop from "../../test-data/order-fix-loop.json";
import none from "../../test-data/order-fix-none.json";
import needed from "../../test-data/order-fix-needed.json";
import OrderFixPanel from "./OrderFixPanel.jsx";

vi.mock("../api.js", () => ({ planOrderFix: vi.fn(), fixOrder: vi.fn() }));
import * as api from "../api.js";

beforeEach(() => vi.resetAllMocks());

describe("OrderFixPanel (rendering real core output)", () => {
  it("changes nothing until the player has seen the plan and confirmed", async () => {
    api.planOrderFix.mockResolvedValue(needed);
    render(<OrderFixPanel cfgPath="/cfg" onFixed={() => {}} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    await screen.findByText("Reorder my mods");
    expect(api.fixOrder).not.toHaveBeenCalled();
    expect(api.planOrderFix).toHaveBeenCalledWith("/cfg");
  });

  it("names the mod that moves up, and shows the whole new order", async () => {
    api.planOrderFix.mockResolvedValue(needed);
    render(<OrderFixPanel cfgPath="/cfg" onFixed={() => {}} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    expect(await screen.findByText(/moves up, from position 3 to 1/)).toBeInTheDocument();
    // Patch and Other only shift down to make room; they are not announced as moves.
    expect(screen.queryByText(/Patch.*moves up/)).not.toBeInTheDocument();
    const order = within(screen.getByLabelText("New order")).getAllByRole("listitem").map((li) => li.textContent);
    expect(order).toEqual(["Lib", "Patch", "Other"]);
    expect(screen.getByText(/Your mods' files are not touched/)).toBeInTheDocument();
  });

  it("reorders on confirm and tells the parent what was done", async () => {
    api.planOrderFix.mockResolvedValue(needed);
    api.fixOrder.mockResolvedValue(needed);
    const onFixed = vi.fn();
    render(<OrderFixPanel cfgPath="/cfg" onFixed={onFixed} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    fireEvent.click(await screen.findByText("Reorder my mods"));
    await waitFor(() => expect(onFixed).toHaveBeenCalledWith(needed));
    expect(api.fixOrder).toHaveBeenCalledWith("/cfg");
  });

  it("cancelling changes nothing", async () => {
    api.planOrderFix.mockResolvedValue(needed);
    render(<OrderFixPanel cfgPath="/cfg" onFixed={() => {}} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    fireEvent.click(await screen.findByText("Cancel"));
    expect(screen.getByText("Fix the order for me")).toBeInTheDocument();
    expect(api.fixOrder).not.toHaveBeenCalled();
  });

  it("explains a loop plainly and offers no reorder", async () => {
    api.planOrderFix.mockResolvedValue(loop);
    render(<OrderFixPanel cfgPath="/cfg" onFixed={() => {}} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    expect(await screen.findByText(/need each other in a loop/)).toHaveTextContent("X, Y");
    expect(screen.queryByText("Reorder my mods")).not.toBeInTheDocument();
  });

  it("says so when moving mods cannot help", async () => {
    api.planOrderFix.mockResolvedValue(none);
    render(<OrderFixPanel cfgPath="/cfg" onFixed={() => {}} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    expect(await screen.findByText(/would not fix the remaining problems/)).toBeInTheDocument();
    expect(screen.queryByText("Reorder my mods")).not.toBeInTheDocument();
  });

  it("shows an error from the core", async () => {
    api.planOrderFix.mockRejectedValue("could not read the profile");
    render(<OrderFixPanel cfgPath="/cfg" onFixed={() => {}} />);
    fireEvent.click(screen.getByText("Fix the order for me"));
    expect(await screen.findByRole("alert")).toHaveTextContent("could not read the profile");
  });
});
