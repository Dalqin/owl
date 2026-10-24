import { describe, expect, it } from "vitest";

import scan from "../test-data/scan-multi-variant.json";
import healthy from "../test-data/profile-view-healthy.json";
import problems from "../test-data/profile-view-problems.json";
import { describeDecisionIssue, describeIssue, describeProblem, describeSource, describeWarning, issuePlugin } from "./describe.js";

describe("describe", () => {
  it("words every warning the core produced for a real scan", () => {
    expect(scan.warnings.length).toBeGreaterThan(0);
    for (const w of scan.warnings) expect(describeWarning(w)).toMatch(/\S/);
  });

  it("names the number of candidates", () => {
    expect(describeWarning({ kind: "multiple_candidates", count: 4 })).toMatch(/4 places/);
  });

  it("throws on kinds it does not know instead of rendering nothing", () => {
    expect(() => describeWarning({ kind: "brand_new" })).toThrow(/warning/);
    expect(() => describeProblem({ kind: "brand_new" })).toThrow(/installer problem/);
    expect(() => describeDecisionIssue({ kind: "brand_new" })).toThrow(/decision issue/);
  });

  it("words every problem the core produced for a real profile", () => {
    expect(problems.check.issues.length).toBeGreaterThan(0);
    for (const i of problems.check.issues) expect(describeIssue(i)).toMatch(/\S/);
  });

  it("names where every plugin in a real load order came from", () => {
    const sources = healthy.check.load_order.map((e) => describeSource(e.source));
    expect(sources).toEqual(["Your OpenMW settings", "Your OpenMW settings", "Simple", "Variant"]);
  });

  it("knows which issues are about a plugin", () => {
    expect(issuePlugin({ kind: "missing_master", plugin: "A.esp", master: "B.esm" })).toBe("A.esp");
    expect(issuePlugin({ kind: "undecided", mod_name: "X" })).toBeNull();
  });

  it("throws on problems and sources it does not know", () => {
    expect(() => describeIssue({ kind: "brand_new" })).toThrow(/problem/);
    expect(() => issuePlugin({ kind: "brand_new" })).toThrow(/problem/);
    expect(() => describeSource({ kind: "brand_new" })).toThrow(/plugin source/);
  });
});
