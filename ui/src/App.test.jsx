import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

import questionOpen from "../test-data/preview-fomod-question.json";
import questionAnswered from "../test-data/preview-fomod-question-answered.json";
import questionScan from "../test-data/scan-fomod-question.json";
import defaults from "../test-data/preview-fomod-default.json";
import chosenB from "../test-data/preview-fomod-b.json";
import fomodView from "../test-data/profile-view-fomod.json";
import healthy from "../test-data/profile-view-healthy.json";
import libraryView from "../test-data/profile-view-library.json";
import notesPreview from "../test-data/preview-notes.json";
import notesScan from "../test-data/scan-notes.json";
import notesView from "../test-data/profile-view-notes.json";
import scan from "../test-data/scan-fomod.json";
import storedA from "../test-data/preview-stored-fomod-a.json";
import storedSame from "../test-data/preview-stored-fomod-same.json";
import { forgetImages } from "./components/ModImage.jsx";
import App from "./App.jsx";

vi.mock("./api.js", () => ({
  locateConfig: vi.fn(),
  chooseConfigFile: vi.fn(),
  chooseFolder: vi.fn(),
  addSource: vi.fn(),
  removeSource: vi.fn(),
  scanFolder: vi.fn(),
  classifyFolder: vi.fn(),
  previewMod: vi.fn(),
  previewStoredMod: vi.fn(),
  addToProfile: vi.fn(),
  rescanMod: vi.fn(),
  readModImage: vi.fn(),
  loadProfile: vi.fn(),
  profilePath: vi.fn(),
  moveInProfile: vi.fn(),
  removeFromProfile: vi.fn(),
  planApply: vi.fn(),
  applyChanges: vi.fn(),
  planOrderFix: vi.fn(),
  fixOrder: vi.fn(),
}));
import * as api from "./api.js";

const empty = { mods: [], check: { issues: [], mods: [], load_order: [] } };

beforeEach(() => {
  vi.resetAllMocks();
  forgetImages();
  api.readModImage.mockRejectedValue("no picture");
  api.locateConfig.mockResolvedValue("/home/u/.config/openmw/openmw.cfg");
  api.chooseFolder.mockResolvedValue("/fixtures/fomod");
  api.scanFolder.mockResolvedValue(scan);
  api.classifyFolder.mockResolvedValue("mod");
  api.addSource.mockResolvedValue(true);
  api.removeSource.mockResolvedValue(undefined);
  api.addToProfile.mockResolvedValue(undefined);
  api.loadProfile.mockResolvedValue(healthy);
  api.profilePath.mockResolvedValue("/p/profile.json");
  api.moveInProfile.mockResolvedValue(undefined);
  api.removeFromProfile.mockResolvedValue(undefined);
  api.previewMod.mockImplementation(async ({ fomodChoices }) =>
    fomodChoices.some((c) => c.option === "B") ? chosenB : defaults,
  );
  api.previewStoredMod.mockImplementation(async ({ fomodChoices }) =>
    fomodChoices.some((c) => c.option === "A") ? storedA : storedSame,
  );
});

const sidebar = () => screen.getByRole("navigation", { name: "Your mods" });
// Opens "Manage my sources" and presses "Add a source", which opens the folder chooser.
const addSource = async () => {
  fireEvent.click(await within(await screen.findByRole("navigation")).findByRole("button", { name: "Manage my sources" }));
  fireEvent.click(await screen.findByRole("button", { name: "Add a source" }));
};

// Presses "Add a source" and stays there; for the folder chooser's own behaviour.
const startAdd = async () => {
  api.loadProfile.mockResolvedValue(empty);
  render(<App />);
  await addSource();
};

// Adds the fixture mod's folder as a source, then opens that mod from the list of sources.
const openAdd = async () => {
  api.loadProfile.mockResolvedValue(empty);
  render(<App />);
  await screen.findByText(/You have no sources yet/);
  api.loadProfile.mockResolvedValue({
    ...empty,
    sources: [{ name: "fomod", path: "/fixtures/fomod", kind: "mod", exists: true, entries: [], truncated: false, in_cfg: false }],
  });
  await addSource();
  fireEvent.click(await screen.findByRole("button", { name: "Open fomod" }));
};

describe("App shell", () => {
  it("shows where openmw.cfg is", async () => {
    render(<App />);
    expect(await screen.findByText(/\/home\/u\/\.config\/openmw\/openmw\.cfg/)).toBeInTheDocument();
  });

  it("says plainly when openmw.cfg cannot be found", async () => {
    api.locateConfig.mockRejectedValue("no openmw.cfg found");
    render(<App />);
    expect(await screen.findByText(/Could not find your OpenMW settings/)).toBeInTheDocument();
  });

  it("lets the player pick an openmw.cfg that was not found, then reloads from it", async () => {
    api.locateConfig.mockRejectedValue("no openmw.cfg found");
    api.chooseConfigFile.mockResolvedValue("D:\\Games\\OpenMW\\openmw.cfg");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Choose the file" }));
    expect(await screen.findByText(/D:\\Games\\OpenMW\\openmw\.cfg/)).toBeInTheDocument();
    await waitFor(() => expect(api.loadProfile).toHaveBeenLastCalledWith("D:\\Games\\OpenMW\\openmw.cfg"));
  });

  it("keeps the current file and says why when the chosen one is refused", async () => {
    api.chooseConfigFile.mockRejectedValue("settings.cfg is not an openmw.cfg file");
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "Use a different file" }));
    expect(await screen.findByText(/is not an openmw\.cfg file/)).toBeInTheDocument();
    expect(screen.getByTestId("cfg-status")).toHaveTextContent("/home/u/.config/openmw/openmw.cfg");
  });

  it("starts on the sources when the list is empty", async () => {
    api.loadProfile.mockResolvedValue(empty);
    render(<App />);
    expect(await screen.findByText(/A source is a place on your computer/)).toBeInTheDocument();
    expect(screen.getByText(/You have no sources yet/)).toBeInTheDocument();
    expect(within(sidebar()).getByRole("button", { name: "Manage my sources" })).toHaveAttribute("aria-current", "page");
    expect(api.chooseFolder).not.toHaveBeenCalled(); // looking is not choosing
  });

  it("starts on all modules when there are mods, with the mods listed in order", async () => {
    render(<App />);
    expect(await screen.findByText("No problems found.")).toBeInTheDocument();
    expect(api.loadProfile).toHaveBeenCalledWith("/home/u/.config/openmw/openmw.cfg");
    const names = within(sidebar()).getAllByRole("listitem").map((li) => li.textContent);
    expect(names).toEqual(["1.Simple", "2.Variant"]);
    expect(within(sidebar()).getByRole("button", { name: "Review and finalize" })).toHaveAttribute("aria-current", "page");
  });

  it("shows a mod's own screen when it is chosen in the list", async () => {
    render(<App />);
    await screen.findByText("No problems found.");
    fireEvent.click(within(sidebar()).getByRole("button", { name: /Variant/ }));
    expect(await screen.findByRole("heading", { name: "Variant" })).toBeInTheDocument();
    expect(within(sidebar()).getByRole("button", { name: /Variant/ })).toHaveAttribute("aria-current", "page");
    fireEvent.click(within(sidebar()).getByRole("button", { name: "Review and finalize" }));
    expect(await screen.findByText("Load order")).toBeInTheDocument();
  });

  it("shows an error from the core when the list cannot be read", async () => {
    api.loadProfile.mockRejectedValue("the profile is damaged");
    render(<App />);
    expect(await screen.findByRole("alert")).toHaveTextContent("the profile is damaged");
  });
});

describe("adding a mod", () => {
  it("scans a chosen folder and shows the installer, outcome and needs", async () => {
    await openAdd();
    expect(await screen.findByText("Installer: Fixture FOMOD")).toBeInTheDocument();
    expect(screen.getByText("What will be set up")).toBeInTheDocument();
    expect(api.scanFolder).toHaveBeenCalledWith("/fixtures/fomod");
    expect(screen.getByTestId("folder-path")).toHaveTextContent("Folder: /fixtures/fomod");
    fireEvent.click(screen.getByRole("tab", { name: "Needs" }));
    expect(screen.getByText("This mod needs these things")).toBeInTheDocument();
  });

  it("sends the player's answer back to the core and shows the new result", async () => {
    await openAdd();
    fireEvent.click(await screen.findByLabelText(/^B/));
    await waitFor(() => expect(screen.getByLabelText(/^B/)).toBeChecked());
    const last = api.previewMod.mock.calls.at(-1)[0];
    expect(last.fomodChoices).toEqual([{ step: "Pick a variant", group: "Variant", option: "B" }]);
    expect(last.name).toBe("fomod");
  });

  it("adds the mod, then shows it selected in the list", async () => {
    await openAdd();
    api.loadProfile.mockResolvedValue(fomodView);
    fireEvent.click(await screen.findByText("Add to my mods"));
    await waitFor(() => expect(api.addToProfile).toHaveBeenCalledWith(defaults.manifest, "/fixtures/fomod"));
    expect(await screen.findByRole("heading", { name: "Fixture" })).toBeInTheDocument();
    expect(within(sidebar()).getByRole("button", { name: /Fixture/ })).toHaveAttribute("aria-current", "page");
  });

  it("will not add a mod until a folder is decided", async () => {
    api.previewMod.mockResolvedValue({
      ...defaults,
      manifest: { ...defaults.manifest, decisions: { ...defaults.manifest.decisions, datapaths: [] } },
    });
    await openAdd();
    expect(await screen.findByText("Add to my mods")).toBeDisabled();
  });

  it("shows an error from the core instead of failing silently", async () => {
    api.scanFolder.mockRejectedValue("/mods/x is not a directory");
    await openAdd();
    expect(await screen.findByRole("alert")).toHaveTextContent("is not a directory");
  });

  it("does nothing when the folder picker is cancelled", async () => {
    api.chooseFolder.mockResolvedValue(null);
    await startAdd();
    await waitFor(() => expect(api.chooseFolder).toHaveBeenCalled());
    expect(api.scanFolder).not.toHaveBeenCalled();
  });

  it("says so when the system's folder chooser cannot open", async () => {
    api.chooseFolder.mockRejectedValue("no portal is running");
    await startAdd();
    expect(await screen.findByRole("alert")).toHaveTextContent("The folder chooser could not open: no portal is running");
    expect(api.scanFolder).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Add a source" })).toBeEnabled(); // can try again
  });
});

describe("changing a mod that is already on the list", () => {
  const open = async () => {
    api.loadProfile.mockResolvedValue(fomodView);
    render(<App />);
    await screen.findByText("1 problem to look at:");
    fireEvent.click(within(sidebar()).getByRole("button", { name: /Fixture/ }));
    await screen.findByText("Installer: Fixture FOMOD");
  };

  it("starts from the saved answers, with nothing to save", async () => {
    await open();
    expect(screen.getByLabelText(/^B/)).toBeChecked();
    expect(screen.getByText("Save changes")).toBeDisabled();
    expect(screen.queryByText("Discard")).not.toBeInTheDocument();
    expect(api.previewStoredMod.mock.calls[0][0].manifest).toEqual(fomodView.mods[0].manifest);
  });

  it("replaces the saved answer for a group instead of adding a second one", async () => {
    await open();
    fireEvent.click(screen.getByRole("radio", { name: /^A/ }));
    await waitFor(() => expect(api.previewStoredMod).toHaveBeenCalledTimes(2));
    // The saved answer had no step ("Variant=B"); it must not survive beside the new one.
    expect(api.previewStoredMod.mock.calls.at(-1)[0].fomodChoices).toEqual([
      { step: "Pick a variant", group: "Variant", option: "A" },
    ]);
  });

  it("saves new answers only when the player says so, and keeps the mod in place", async () => {
    await open();
    fireEvent.click(screen.getByRole("radio", { name: /^A/ }));
    const save = await screen.findByText("Save changes");
    await waitFor(() => expect(save).toBeEnabled());
    expect(screen.getByText("These answers are not saved yet.")).toBeInTheDocument();
    expect(api.addToProfile).not.toHaveBeenCalled();
    fireEvent.click(save);
    await waitFor(() => expect(api.addToProfile).toHaveBeenCalledWith(storedA.manifest, "/fixtures/fomod"));
    await waitFor(() => expect(api.loadProfile).toHaveBeenCalledTimes(2));
  });

  it("goes back to the saved answers on Discard", async () => {
    await open();
    fireEvent.click(screen.getByRole("radio", { name: /^A/ }));
    fireEvent.click(await screen.findByText("Discard"));
    await waitFor(() => expect(screen.getByLabelText(/^B/)).toBeChecked());
    expect(screen.getByText("Save changes")).toBeDisabled();
    expect(api.addToProfile).not.toHaveBeenCalled();
  });

  it("will not move or remove the mod while there are unsaved answers", async () => {
    await open();
    fireEvent.click(screen.getByRole("radio", { name: /^A/ }));
    await screen.findByText("Discard");
    expect(screen.getByLabelText("Remove Fixture")).toBeDisabled();
    expect(screen.getByLabelText("Move Fixture up")).toBeDisabled();
  });

  it("still lets the player remove a mod whose saved answers are out of date", async () => {
    // Nothing was typed, but the installer now leads somewhere other than what was saved.
    api.previewStoredMod.mockResolvedValue({ ...(await api.previewStoredMod({ fomodChoices: [] })), changed: true });
    await open();
    expect(screen.getByText("Save changes")).toBeEnabled();
    expect(screen.getByText(/reads this mod's installer differently/)).toBeInTheDocument();
    expect(screen.queryByText("Discard")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Remove Fixture")).toBeEnabled();
    expect(screen.getByText("Refresh")).toBeEnabled();
  });

  it("moves a mod and keeps it selected at its new position", async () => {
    api.loadProfile.mockResolvedValue(healthy);
    render(<App />);
    await screen.findByText("No problems found.");
    fireEvent.click(within(sidebar()).getByRole("button", { name: /Simple/ }));
    await screen.findByRole("heading", { name: "Simple" });
    expect(screen.getByLabelText("Move Simple up")).toBeDisabled();
    // After the move the list comes back with the two mods swapped.
    api.loadProfile.mockResolvedValue({ ...healthy, mods: [...healthy.mods].reverse(), check: { ...healthy.check, mods: [...healthy.check.mods].reverse() } });
    fireEvent.click(screen.getByLabelText("Move Simple down"));
    await waitFor(() => expect(api.moveInProfile).toHaveBeenCalledWith(0, 1));
    await waitFor(() =>
      expect(within(sidebar()).getByRole("button", { name: /Simple/ })).toHaveAttribute("aria-current", "page"),
    );
    expect(within(sidebar()).getAllByRole("listitem").map((li) => li.textContent)).toEqual(["1.Variant", "2.Simple"]);
  });

  it("removes the mod and goes back to all modules", async () => {
    await open();
    api.loadProfile.mockResolvedValue(empty);
    fireEvent.click(screen.getByLabelText("Remove Fixture"));
    await waitFor(() => expect(api.removeFromProfile).toHaveBeenCalledWith(0));
    expect(await screen.findByText("You have not added any mods yet.")).toBeInTheDocument();
  });

  it("shows an error from the core and keeps the screen", async () => {
    await open();
    api.removeFromProfile.mockRejectedValue("there is no mod at position 3");
    fireEvent.click(screen.getByLabelText("Remove Fixture"));
    expect(await screen.findByRole("alert")).toHaveTextContent("no mod at position 3");
    expect(screen.getByText("Installer: Fixture FOMOD")).toBeInTheDocument();
  });

  it("shows the problems that concern this mod, in words", async () => {
    await open();
    // The sample list has one problem, about this mod's plugin.
    expect(screen.getByRole("note", { name: "" })).toHaveTextContent(/B\.esp/);
  });
});

describe("the author's notes", () => {
  it("can be read before a mod is added", async () => {
    api.scanFolder.mockResolvedValue(notesScan);
    api.previewMod.mockResolvedValue(notesPreview);
    await openAdd();
    fireEvent.click(await screen.findByRole("tab", { name: "Author's notes" }));
    expect(await screen.findByText(/Requirements:/)).toBeInTheDocument();
    expect(screen.getByText("An Author")).toBeInTheDocument();
    // Reading them does not add anything.
    expect(api.addToProfile).not.toHaveBeenCalled();
  });

  it("are on the screen of a mod already on the list", async () => {
    api.loadProfile.mockResolvedValue(notesView);
    api.previewStoredMod.mockResolvedValue({ ...storedSame, manifest: notesView.mods[0].manifest, fomod: null, changed: false });
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: /Notes/ }));
    fireEvent.click(await screen.findByRole("tab", { name: "Author's notes" }));
    expect(await screen.findByText("README.txt")).toBeInTheDocument();
  });

  it("can be picked up again from the folder, and says which choices no longer match", async () => {
    api.loadProfile.mockResolvedValue(notesView);
    api.previewStoredMod.mockResolvedValue({ ...storedSame, manifest: notesView.mods[0].manifest, fomod: null, changed: false });
    api.rescanMod.mockResolvedValue([{ kind: "plugin_missing", name: "Gone.esp" }]);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: /Notes/ }));
    fireEvent.click(await screen.findByText("Refresh"));
    await waitFor(() => expect(api.rescanMod).toHaveBeenCalledWith(0));
    await waitFor(() => expect(api.loadProfile).toHaveBeenCalledTimes(2));
    expect(await screen.findByRole("status")).toHaveTextContent(/One of your choices no longer matches it: .*Gone\.esp/);
  });

  it("says when the folder still matches", async () => {
    api.loadProfile.mockResolvedValue(notesView);
    api.previewStoredMod.mockResolvedValue({ ...storedSame, manifest: notesView.mods[0].manifest, fomod: null, changed: false });
    api.rescanMod.mockResolvedValue([]);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: /Notes/ }));
    fireEvent.click(await screen.findByText("Refresh"));
    expect(await screen.findByText("Refreshed")).toBeInTheDocument();
  });
});

describe("the author's pictures", () => {
  it("shows the installer's banner and an option's picture, read from the mod's own folder", async () => {
    api.readModImage.mockImplementation(async (_root, path) => `data:image/png;base64,${path.endsWith("banner.png") ? "AAAA" : "BBBB"}`);
    await openAdd();
    expect(await screen.findByRole("img", { name: /banner from the author/ })).toHaveAttribute("src", "data:image/png;base64,BBBB");
    expect(await screen.findByRole("img", { name: /A: picture from the author/ })).toBeInTheDocument();
    expect(api.readModImage).toHaveBeenCalledWith("/fixtures/fomod", "fomod/Banner.PNG");
    expect(api.readModImage).toHaveBeenCalledWith("/fixtures/fomod", "fomod/banner.png");
  });

  it("is fine when the pictures are missing", async () => {
    await openAdd();
    expect(await screen.findByText("Installer: Fixture FOMOD")).toBeInTheDocument();
    expect(screen.queryByRole("img")).not.toBeInTheDocument();
  });
});

describe("a folder that is one mod", () => {
  it("becomes a source and is listed, and the player stays on the sources", async () => {
    api.loadProfile.mockResolvedValue(empty);
    render(<App />);
    await screen.findByText(/You have no sources yet/);
    api.loadProfile.mockResolvedValue({
      ...empty,
      sources: [{ name: "fomod", path: "/fixtures/fomod", kind: "mod", exists: true, entries: [], truncated: false, in_cfg: false }],
    });
    await addSource();
    await waitFor(() => expect(api.addSource).toHaveBeenCalledWith("/fixtures/fomod", "mod"));
    expect(await screen.findByText("Added “fomod” as a source")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "My sources" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open fomod" })).toBeInTheDocument(); // listed, not opened
    expect(within(sidebar()).getByRole("button", { name: "fomod" })).not.toHaveAttribute("aria-current", "page");
    expect(api.scanFolder).not.toHaveBeenCalled();
  });

  it("says when the folder was already a source", async () => {
    api.loadProfile.mockResolvedValue(empty);
    api.addSource.mockResolvedValue(false);
    render(<App />);
    await addSource();
    expect(await screen.findByText("“fomod” is already a source")).toBeInTheDocument();
  });

  it("can be forgotten from the left list without adding it", async () => {
    api.loadProfile.mockResolvedValue({
      ...empty,
      sources: [{ name: "fomod", path: "/fixtures/fomod", kind: "mod", exists: true, entries: [], truncated: false, in_cfg: false }],
    });
    render(<App />);
    fireEvent.click(await within(await screen.findByRole("navigation")).findByRole("button", { name: "Stop listing fomod" }));
    await waitFor(() => expect(api.removeSource).toHaveBeenCalledWith("/fixtures/fomod"));
  });
});

describe("a folder that holds many mods", () => {
  const chooseLibrary = async () => {
    api.loadProfile.mockResolvedValue(empty);
    api.chooseFolder.mockResolvedValue("/fixtures/library");
    api.classifyFolder.mockResolvedValue("library");
    render(<App />);
    await addSource();
    await screen.findByText("This folder holds several mods");
  };

  it("asks what to do with it, and does not read it as one mod", async () => {
    await chooseLibrary();
    expect(api.classifyFolder).toHaveBeenCalledWith("/fixtures/library");
    expect(api.scanFolder).not.toHaveBeenCalled();
    expect(api.addSource).not.toHaveBeenCalled();
  });

  it("remembers it and shows its mods in the list, and stays on the sources", async () => {
    await chooseLibrary();
    api.loadProfile.mockResolvedValue(libraryView);
    fireEvent.click(screen.getByText("Show its mods in my list"));
    await waitFor(() => expect(api.addSource).toHaveBeenCalledWith("/fixtures/library", "library"));
    expect(await screen.findByText("Added “library” as a source")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "My sources" })).toBeInTheDocument();
    expect(screen.queryByText("This folder holds several mods")).not.toBeInTheDocument();
    expect(screen.queryByRole("heading", { name: "library" })).not.toBeInTheDocument(); // not opened
    expect(within(sidebar()).getByRole("button", { name: /^library/ })).not.toHaveAttribute("aria-current", "page");
    // Only the mods that are not on the list yet are offered in the left list.
    expect(within(sidebar()).getByRole("button", { name: "Beta Installer" })).toBeInTheDocument();
    expect(within(sidebar()).queryByRole("button", { name: "Alpha Mod" })).not.toBeInTheDocument();
    expect(within(sidebar()).queryByRole("button", { name: "Packed.7z" })).not.toBeInTheDocument();
  });

  it("can be read as one mod instead, when the player says it is one", async () => {
    await chooseLibrary();
    fireEvent.click(screen.getByText("It is one mod"));
    await waitFor(() => expect(api.addSource).toHaveBeenCalledWith("/fixtures/library", "mod"));
    expect(api.addSource).not.toHaveBeenCalledWith("/fixtures/library", "library");
    expect(await screen.findByText("Added “library” as a source")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "My sources" })).toBeInTheDocument();
    expect(api.scanFolder).not.toHaveBeenCalled(); // listed, not opened
  });

  it("lists everything in the folder, says what each is, and offers to open only what can be added", async () => {
    api.loadProfile.mockResolvedValue(libraryView);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: /^library/ }));
    const entries = (await screen.findAllByRole("listitem")).filter((li) => li.classList.contains("library-entry"));
    expect(entries.map((li) => li.querySelector(".library-name").textContent)).toEqual(["Alpha Mod", "Beta Installer", "Docs", "Packed.7z"]);
    expect(within(entries[0]).getByText("On your list")).toBeInTheDocument();
    // The sample's own settings already point at Beta Installer.
    expect(within(entries[1]).getByText("Already in your OpenMW settings, not added by Owl")).toBeInTheDocument();
    expect(within(entries[1]).getByLabelText("Open Beta Installer")).toBeInTheDocument();
    expect(within(entries[2]).getByText("No mod found in this folder.")).toBeInTheDocument();
    expect(within(entries[3]).getByText(/Extract it first/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Open Alpha Mod")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Open Packed.7z")).not.toBeInTheDocument();
  });

  it("works through a mod from the library and adds it, which takes it off the available list", async () => {
    api.loadProfile.mockResolvedValue(libraryView);
    api.scanFolder.mockResolvedValue(scan);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: "Beta Installer" }));
    expect(await screen.findByText("Installer: Fixture FOMOD")).toBeInTheDocument();
    expect(api.scanFolder).toHaveBeenCalledWith("/fixtures/library/Beta Installer");

    const added = structuredClone(libraryView);
    added.sources[0].entries[1].added = true;
    added.mods.push({ ...fomodView.mods[0], root: "/fixtures/library/Beta Installer" });
    added.check.mods.push(fomodView.check.mods[0]);
    api.loadProfile.mockResolvedValue(added);
    fireEvent.click(await screen.findByText("Add to my mods"));
    await waitFor(() => expect(api.addToProfile).toHaveBeenCalledWith(defaults.manifest, "/fixtures/library/Beta Installer"));
    expect(await screen.findByRole("heading", { name: "Fixture" })).toBeInTheDocument();
    expect(within(sidebar()).queryByRole("button", { name: "Beta Installer" })).not.toBeInTheDocument();
  });

  it("says plainly when the folder is not there any more", async () => {
    api.loadProfile.mockResolvedValue(libraryView);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: /^no-such-library/ }));
    expect(await screen.findByText(/not there any more/)).toBeInTheDocument();
  });

  it("stops listing a folder without touching the mods already added", async () => {
    api.loadProfile.mockResolvedValue(libraryView);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: /^library/ }));
    api.loadProfile.mockResolvedValue({ ...libraryView, sources: libraryView.sources.slice(1) });
    fireEvent.click(within(sidebar()).getByRole("button", { name: "Stop listing library" }));
    await waitFor(() => expect(api.removeSource).toHaveBeenCalledWith("/fixtures/library"));
    expect(await screen.findByText(/A source is a place on your computer/)).toBeInTheDocument();
    expect(api.removeFromProfile).not.toHaveBeenCalled();
    expect(within(sidebar()).getByRole("button", { name: /Alpha Mod/ })).toBeInTheDocument(); // still on the list
  });
});

describe("managing sources", () => {
  const open = async () => {
    api.loadProfile.mockResolvedValue(libraryView);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: "Manage my sources" }));
    await screen.findByRole("heading", { name: "My sources" });
  };
  const rows = () => screen.getAllByRole("listitem").filter((li) => li.classList.contains("library-entry"));

  it("explains what a source is, and that removing one changes nothing else", async () => {
    await open();
    expect(screen.getByText(/A source is a place on your computer where Owl looks for mods/)).toBeInTheDocument();
    expect(screen.getByText(/does not delete any files, change your OpenMW settings/)).toBeInTheDocument();
  });

  it("lists every source with where it is and what is in it", async () => {
    await open();
    const text = rows().map((li) => [li.querySelector(".library-name").textContent, li.querySelector(".library-status").textContent]);
    expect(text).toEqual([
      ["library", "2 mods in this folder, 1 not on your list yet."],
      ["no-such-library", "This folder is not there any more. It may be on a drive that is not connected."],
      ["simple", "One mod that is not on your list yet."],
      ["no-such-mod", "This folder is not there any more. It may be on a drive that is not connected."],
    ]);
    expect(within(rows()[0]).getByText("/fixtures/library")).toBeInTheDocument();
    // A folder that is gone cannot be opened, but it can still be removed.
    expect(within(rows()[1]).queryByRole("button", { name: /^Open/ })).not.toBeInTheDocument();
    expect(within(rows()[1]).getByRole("button", { name: "Remove source no-such-library" })).toBeInTheDocument();
  });

  it("opens a source from the list", async () => {
    await open();
    fireEvent.click(screen.getByRole("button", { name: "Open library" }));
    expect(await screen.findByRole("heading", { name: "library" })).toBeInTheDocument();
  });

  it("removes a source and stays on the sources, without touching openmw.cfg or the list of mods", async () => {
    await open();
    api.loadProfile.mockResolvedValue({ ...libraryView, sources: libraryView.sources.filter((s) => s.path !== "/fixtures/library") });
    fireEvent.click(screen.getByRole("button", { name: "Remove source library" }));
    await waitFor(() => expect(api.removeSource).toHaveBeenCalledWith("/fixtures/library"));
    await waitFor(() => expect(rows().map((li) => li.querySelector(".library-name").textContent)).not.toContain("library"));
    expect(screen.getByRole("heading", { name: "My sources" })).toBeInTheDocument();
    expect(api.planApply).not.toHaveBeenCalled();
    expect(api.applyChanges).not.toHaveBeenCalled();
    expect(api.removeFromProfile).not.toHaveBeenCalled();
  });

  it("says when a mod in a source is already in the player's OpenMW settings", async () => {
    const view = structuredClone(libraryView);
    view.sources[2].in_cfg = true;
    api.loadProfile.mockResolvedValue(view);
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: "Manage my sources" }));
    expect(await screen.findByText("One mod, already in your OpenMW settings (not added by Owl).")).toBeInTheDocument();
  });
});

describe("an installer question with no default", () => {
  const openQuestion = async () => {
    api.scanFolder.mockResolvedValue(questionScan);
    api.previewMod.mockImplementation(async ({ fomodChoices }) =>
      fomodChoices.some((c) => c.option === "B") ? questionAnswered : questionOpen,
    );
    await openAdd();
    await screen.findByText("Installer: Question FOMOD");
  };

  it("is marked where it is asked, and is not shown as a bare note", async () => {
    await openQuestion();
    const group = screen.getByRole("group", { name: "Variant" });
    expect(group).toHaveClass("needs-answer");
    expect(within(group).getByText(/An answer is needed here/)).toBeInTheDocument();
    expect(within(group).getAllByRole("radio")).toHaveLength(2);
    // The old wording, which read like a form field, is gone.
    expect(screen.queryByText(/a choice is needed: A, B/)).not.toBeInTheDocument();
  });

  it("is listed under what will be set up, with a way to reach it", async () => {
    await openQuestion();
    const list = screen.getByRole("note", { name: "Still to answer" });
    expect(list).toHaveTextContent("Pick a variant → Variant");
    fireEvent.click(within(list).getByText("Go to this question"));
    expect(screen.getAllByRole("radio")[0]).toHaveFocus();
  });

  it("keeps the mod from being added until it is answered, and says what is left", async () => {
    await openQuestion();
    expect(screen.getByText("Add to my mods")).toBeDisabled();
    expect(screen.getByText(/question to answer before this can be added/)).toHaveTextContent("1 question to answer before this can be added: Variant.");
    fireEvent.click(screen.getByRole("radio", { name: /^B/ }));
    await waitFor(() => expect(screen.getByText("Add to my mods")).toBeEnabled());
    expect(screen.queryByRole("note", { name: "Still to answer" })).not.toBeInTheDocument();
    expect(screen.queryByText(/An answer is needed here/)).not.toBeInTheDocument();
  });

  it("also keeps a mod already on the list from being saved while a question is open", async () => {
    const stored = structuredClone(fomodView);
    stored.mods[0].manifest = questionOpen.manifest;
    api.loadProfile.mockResolvedValue(stored);
    api.previewStoredMod.mockResolvedValue({ ...questionOpen, changed: true });
    render(<App />);
    await screen.findByText(/problems? to look at:|No problems found\./);
    fireEvent.click(within(sidebar()).getByRole("button", { name: /Question/ }));
    await screen.findByText("Installer: Question FOMOD");
    expect(screen.getByText("Save changes")).toBeDisabled();
    expect(screen.getByRole("status")).toHaveTextContent("before this can be saved: Variant.");
  });

  it("does not stop other installer problems from being shown as notes", async () => {
    api.scanFolder.mockResolvedValue(questionScan);
    const odd = structuredClone(questionAnswered);
    odd.fomod.problems = [{ kind: "source_not_found", source: "Gone" }];
    api.previewMod.mockResolvedValue(odd);
    await openAdd();
    expect(await screen.findByText(/Gone: the installer refers to a folder/)).toBeInTheDocument();
  });
});
