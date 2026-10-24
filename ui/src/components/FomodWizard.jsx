import { unknown } from "../unknown.js";
import ModImage from "./ModImage.jsx";
import OptionDescription from "./OptionDescription.jsx";

// How a group's options are chosen. "all" groups install everything and ask nothing.
function inputKind(kind) {
  switch (kind) {
    case "select_exactly_one":
    case "select_at_most_one":
      return "radio";
    case "select_any":
    case "select_at_least_one":
      return "checkbox";
    case "select_all":
      return "none";
    default:
      return unknown("group kind", kind);
  }
}

const KIND_HELP = {
  select_exactly_one: "Choose one",
  select_at_most_one: "Choose at most one",
  select_any: "Choose any",
  select_at_least_one: "Choose at least one",
  select_all: "All of these are installed",
};

// Where a question lives on the page, so something else can send the player to it.
export const questionId = (step, group) => `question|${step}|${group}`;

// The mod author's installer, as plain questions. `resolution` says which steps
// are currently shown and what is selected; `onChange(step, group, optionNames)`
// reports the full new selection for one group. `root` is the mod's folder,
// where the author's pictures are.
export default function FomodWizard({ info, resolution, onChange, root, unanswered = [] }) {
  const visible = new Set(resolution.visible_steps);
  const isOpen = (step, group) => unanswered.some((q) => q.step === step && q.group === group);
  const isSelected = (step, group, option) =>
    resolution.selected.some((s) => s.step === step && s.group === group && s.option === option);

  return (
    <section aria-labelledby="installer-heading">
      <h3 id="installer-heading">Installer: {info.module_name ?? "this mod"}</h3>
      <p className="muted">The mod's author set up these questions. Your answers decide which files are used.</p>
      {info.steps
        .filter((step) => visible.has(step.name))
        .map((step, si) => (
          <fieldset key={`${si}-${step.name}`} className="step">
            <legend>{step.name}</legend>
            {step.groups.map((group, gi) => {
              const input = inputKind(group.kind);
              const current = group.options.filter((o) => isSelected(step.name, group.name, o.name)).map((o) => o.name);
              const open = isOpen(step.name, group.name);
              return (
                <div
                  key={`${gi}-${group.name}`}
                  id={questionId(step.name, group.name)}
                  className={`group ${open ? "needs-answer" : ""}`}
                  role="group"
                  aria-label={group.name}
                >
                  <div className="group-title">
                    {group.name} <span className="muted">({KIND_HELP[group.kind]})</span>
                  </div>
                  {open && (
                    <p className="group-needs" role="note">
                      An answer is needed here before this mod can be added.
                    </p>
                  )}
                  {group.options.map((option, oi) => {
                    const selected = current.includes(option.name);
                    const unavailable = option.default_type === "not_usable" && input !== "none";
                    const id = `${step.name}|${group.name}|${option.name}`;
                    return (
                      <div key={`${oi}-${option.name}`} className={`option ${selected ? "selected" : ""}`}>
                        {input === "none" ? (
                          <span aria-hidden="true">•</span>
                        ) : (
                          <input
                            id={id}
                            type={input}
                            name={`${step.name}|${group.name}`}
                            checked={selected}
                            disabled={unavailable}
                            onChange={(e) => {
                              const next =
                                input === "radio"
                                  ? [option.name]
                                  : e.target.checked
                                    ? [...current, option.name]
                                    : current.filter((n) => n !== option.name);
                              onChange(step.name, group.name, next);
                            }}
                          />
                        )}
                        <span className="option-body">
                          {/* The label is only the name, so the description's button can sit beside it without selecting the option. */}
                          <label className="option-name" htmlFor={input === "none" ? undefined : id}>
                            {option.name}
                            {option.default_type === "recommended" && input !== "none" && (
                              <span className="tag">recommended</span>
                            )}
                            {unavailable && <span className="tag">unavailable</span>}
                          </label>
                          <OptionDescription name={option.name} text={option.description} />
                          {option.image && (
                            <ModImage root={root} path={option.image} alt={`${option.name}: picture from the author`} className="option-image" />
                          )}
                        </span>
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </fieldset>
        ))}
    </section>
  );
}
