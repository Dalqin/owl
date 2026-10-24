import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import ModImage, { forgetImages } from "./ModImage.jsx";

vi.mock("../api.js", () => ({ readModImage: vi.fn() }));
import * as api from "../api.js";

beforeEach(() => {
  vi.resetAllMocks();
  forgetImages();
});

describe("ModImage", () => {
  it("shows the author's picture once the core has read it", async () => {
    api.readModImage.mockResolvedValue("data:image/png;base64,AAAA");
    render(<ModImage root="/mods/a" path="fomod/banner.png" alt="Banner" />);
    const img = await screen.findByRole("img", { name: "Banner" });
    expect(img).toHaveAttribute("src", "data:image/png;base64,AAAA");
    expect(api.readModImage).toHaveBeenCalledWith("/mods/a", "fomod/banner.png");
  });

  it("shows nothing, and no error, when the picture cannot be read", async () => {
    api.readModImage.mockRejectedValue("no such picture in the mod's folder");
    render(<ModImage root="/mods/a" path="gone.png" alt="Gone" />);
    await waitFor(() => expect(api.readModImage).toHaveBeenCalled());
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });

  it("asks only once for a picture that many options share", async () => {
    api.readModImage.mockResolvedValue("data:image/png;base64,AAAA");
    render(
      <>
        <ModImage root="/mods/a" path="shared.png" alt="One" />
        <ModImage root="/mods/a" path="shared.png" alt="Two" />
      </>,
    );
    await screen.findByRole("img", { name: "One" });
    expect(await screen.findByRole("img", { name: "Two" })).toBeInTheDocument();
    expect(api.readModImage).toHaveBeenCalledTimes(1);
  });

  it("does not remember a failure, so a later try can succeed", async () => {
    api.readModImage.mockRejectedValueOnce("busy").mockResolvedValue("data:image/png;base64,BBBB");
    const first = render(<ModImage root="/mods/a" path="x.png" alt="X" />);
    await waitFor(() => expect(api.readModImage).toHaveBeenCalledTimes(1));
    await Promise.resolve();
    first.unmount();
    render(<ModImage root="/mods/a" path="x.png" alt="X again" />);
    expect(await screen.findByRole("img", { name: "X again" })).toBeInTheDocument();
  });
});
