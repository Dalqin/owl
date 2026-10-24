import { useEffect, useState } from "react";

import * as api from "../api.js";

// The same picture is often used by many options, so each one is asked for once.
// Failures are not remembered, and the oldest pictures are dropped past a limit.
const LIMIT = 40;
const loaded = new Map(); // "root\0path" -> Promise of a data: address

export function forgetImages() {
  loaded.clear();
}

function load(root, path) {
  const key = `${root}\0${path}`;
  if (!loaded.has(key)) {
    const promise = api.readModImage(root, path);
    promise.catch(() => loaded.delete(key));
    loaded.set(key, promise);
    if (loaded.size > LIMIT) loaded.delete(loaded.keys().next().value);
  }
  return loaded.get(key);
}

// A picture the mod's author put in the installer. If it cannot be read there is
// nothing to show, and nothing is made up in its place.
export default function ModImage({ root, path, alt, className }) {
  const [src, setSrc] = useState(null);
  useEffect(() => {
    let current = true;
    setSrc(null);
    load(root, path).then(
      (uri) => current && setSrc(uri),
      () => {},
    );
    return () => {
      current = false;
    };
  }, [root, path]);
  return src ? <img className={className} src={src} alt={alt} /> : null;
}
