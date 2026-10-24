import { useEffect, useState } from "react";

import * as api from "./api.js";
import ModWorkspace from "./components/ModWorkspace.jsx";

const folderName = (path) => path.split(/[\\/]/).filter(Boolean).pop() ?? "mod";

// Reads one mod folder and lets the player answer its installer and add it to the
// list. Used for a folder chosen by hand and for a mod found in a library.
export default function ScannedMod({ folder, cfg, onAdded }) {
  const [report, setReport] = useState(null);
  const [error, setError] = useState(null);

  useEffect(() => {
    let current = true;
    setReport(null);
    setError(null);
    api.scanFolder(folder).then(
      (r) => current && setReport(r),
      (e) => current && setError(String(e)),
    );
    return () => {
      current = false;
    };
  }, [folder]);

  if (error) return <p className="note error" role="alert">{error}</p>;
  if (!report) return <p className="muted">Looking…</p>;

  return (
    <ModWorkspace
      newMod
      title={folderName(folder)}
      folder={folder}
      fomod={report.fomod}
      candidates={report.datapath_candidates}
      warnings={report.warnings}
      initial={{ choices: [], datapaths: [] }}
      loadPreview={({ choices, datapaths }) =>
        api.previewMod({
          report,
          name: folderName(folder),
          fomodChoices: choices,
          datapaths,
          plugins: [],
          cfgPath: cfg.path,
        })
      }
      saveLabel="Add to my mods"
      onSave={async (manifest) => {
        await api.addToProfile(manifest, folder);
        await onAdded(folder);
      }}
    />
  );
}
