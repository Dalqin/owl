import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import scan from "../../test-data/scan-fomod.json";
import OptionDescription from "./OptionDescription.jsx";

// The second option's description in the real sample is multi-line with \r\n endings.
const multi = scan.fomod.steps[0].groups[0].options[1].description;

describe("OptionDescription", () => {
  it("shows only the first line until asked for more", () => {
    render(<OptionDescription name="B" text={multi} />);
    expect(screen.getByText("The second variant")).toBeInTheDocument();
    expect(screen.queryByText(/Example mod/)).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Read more about B" })).toHaveAttribute("aria-expanded", "false");
  });

  it("reveals the author's whole text, line breaks kept, and can collapse again", () => {
    render(<OptionDescription name="B" text={multi} />);
    fireEvent.click(screen.getByRole("button", { name: "Read more about B" }));
    const full = screen.getByText(/Use this one if you also use the Example mod\./);
    expect(full.textContent).toBe(
      "The second variant\n\nUse this one if you also use the Example mod.\nIt avoids changing the same records.",
    );
    const button = screen.getByRole("button", { name: "Show less about B" });
    expect(button).toHaveAttribute("aria-expanded", "true");
    fireEvent.click(button);
    expect(screen.queryByText(/Example mod/)).not.toBeInTheDocument();
  });

  it("offers no button for a one-line description", () => {
    render(<OptionDescription name="A" text="The first variant" />);
    expect(screen.getByText("The first variant")).toBeInTheDocument();
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("ignores trailing blank lines when deciding there is more", () => {
    render(<OptionDescription name="A" text={"Just this.\r\n\r\n\r\n  "} />);
    expect(screen.queryByRole("button")).not.toBeInTheDocument();
  });

  it("skips decoration lines when picking the first line, but keeps them in the full text", () => {
    const text = "*****************\r\nReal first line\r\nmore";
    render(<OptionDescription name="X" text={text} />);
    expect(screen.getByText("Real first line")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /Read more/ }));
    expect(screen.getByText(/\*{5}/).textContent).toContain("*****************");
  });

  it("renders nothing for an empty description", () => {
    const { container } = render(<OptionDescription name="X" text={"  \r\n "} />);
    expect(container).toBeEmptyDOMElement();
  });
});
