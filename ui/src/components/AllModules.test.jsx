import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import healthy from "../../test-data/profile-view-healthy.json";
import orderFixNeeded from "../../test-data/order-fix-needed.json";
import misordered from "../../test-data/profile-view-misordered.json";
import problems from "../../test-data/profile-view-problems.json";
import AllModules from "./AllModules.jsx";

vi.mock("../api.js", () => ({
  planApply: vi.fn(),
  applyChanges: vi.fn(),
  planOrderFix: vi.fn(),
  fixOrder: vi.fn(),
}));
import * as api from "../api.js";

beforeEach(() => vi.resetAllMocks());

const show = (view, props = {}) =>
  render(
    <AllModules
      view={view}
      cfgPath="/cfg"
      profileFile="/home/u/.local/share/owl/profile.json"
      onReload={() => {}}
      onAdd={() => {}}
      {...props}
    />,
  );

describe("AllModules (rendering real core output)", () => {
  it("says when nothing is wrong", () => {
    show(healthy);
    expect(screen.getByText("No problems found.")).toBeInTheDocument();
  });

  it("shows the plugin load order, yours first, then each mod's", () => {
    show(healthy);
    const rows = screen.getAllByRole("row").slice(1); // skip header
    const cells = rows.map((r) => within(r).getAllByRole("cell").map((c) => c.textContent));
    expect(cells.map((c) => c[1])).toEqual(["Morrowind.esm", "Tribunal.esm", "Simple.esp", "Variant.ESP"]);
    expect(cells[0][2]).toBe("Your OpenMW settings");
    expect(cells[2][2]).toBe("Simple");
  });

  it("explains each problem in words and flags the plugin it concerns", () => {
    show(problems);
    expect(screen.getByText("3 problems to look at:")).toBeInTheDocument();
    expect(screen.getByText(/Undecided: no folder is chosen yet/)).toBeInTheDocument();
    expect(screen.getByText(/Gone: its folder is missing/)).toBeInTheDocument();
    const row = screen.getByRole("row", { name: /Variant\.ESP/ });
    expect(within(row).getByText(/needs Tribunal\.esm, which is not loaded/)).toBeInTheDocument();
  });

  it("invites adding a mod when the list is empty", () => {
    const onAdd = vi.fn();
    show({ mods: [], check: { issues: [], mods: [], load_order: [] } }, { onAdd });
    expect(screen.getByText("You have not added any mods yet.")).toBeInTheDocument();
    fireEvent.click(screen.getByText("Add a mod"));
    expect(onAdd).toHaveBeenCalled();
  });

  it("tells the player where the list is saved", () => {
    show(healthy);
    expect(screen.getByText(/\/home\/u\/\.local\/share\/owl\/profile\.json/)).toBeInTheDocument();
  });

  it("offers to fix the order only when plugins load before ones they need", () => {
    show(misordered);
    expect(screen.getByText("Fix the order for me")).toBeInTheDocument();
    // In the problem list, and again beside the plugin's row in the load order.
    expect(screen.getAllByText(/needs Lib\.esp, but Lib\.esp loads after it/)).toHaveLength(2);
  });

  it("does not offer it when the problems are of another kind, or there are none", () => {
    const { unmount } = show(problems);
    expect(screen.queryByText("Fix the order for me")).not.toBeInTheDocument();
    unmount();
    show(healthy);
    expect(screen.queryByText("Fix the order for me")).not.toBeInTheDocument();
  });

  it("after fixing, re-reads the list and says what happened", async () => {
    const onReload = vi.fn();
    api.planOrderFix.mockResolvedValue({ ...orderFixNeeded });
    api.fixOrder.mockResolvedValue({ ...orderFixNeeded, changed: true });
    show(misordered, { onReload });
    fireEvent.click(screen.getByText("Fix the order for me"));
    fireEvent.click(await screen.findByText("Reorder my mods"));
    expect(await screen.findByText(/Your list is reordered/)).toBeInTheDocument();
    await waitFor(() => expect(onReload).toHaveBeenCalledTimes(1));
  });
});
