// A missing or renamed field in JavaScript is just `undefined`, which renders as
// nothing. Anything that switches on data from the Rust side calls this in its
// default branch so drift becomes a visible error instead of a silent blank.
export function unknown(what, value) {
  throw new Error(`Unexpected ${what} from the app core: ${JSON.stringify(value)}`);
}
