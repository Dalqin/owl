// Small line icons for the sidebar. They are decoration: every row already says
// what it is in words, so they are hidden from screen readers.
const base = {
  width: 16,
  height: 16,
  viewBox: "0 0 16 16",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.4,
  strokeLinecap: "round",
  strokeLinejoin: "round",
  "aria-hidden": "true",
  focusable: "false",
  className: "icon",
};

// A folder on disk that holds mods.
export const FolderIcon = () => (
  <svg {...base}>
    <path d="M1.8 4.2a1 1 0 0 1 1-1h3l1.4 1.6h6a1 1 0 0 1 1 1v6.2a1 1 0 0 1-1 1H2.8a1 1 0 0 1-1-1z" />
  </svg>
);

// One mod.
export const ModIcon = () => (
  <svg {...base}>
    <path d="M8 1.8 13.6 4.6v6.8L8 14.2 2.4 11.4V4.6z" />
    <path d="M2.6 4.7 8 7.5l5.4-2.8M8 7.5v6.6" />
  </svg>
);

// A question mark in a circle, for the help page.
export const HelpIcon = () => (
  <svg {...base}>
    <circle cx="8" cy="8" r="6.2" />
    <path d="M6.2 6.2a1.9 1.9 0 1 1 2.6 1.8c-.6.3-.8.6-.8 1.2M8 11.4v.1" />
  </svg>
);

// A group of mods picked one at a time, not from a folder.
export const PickedIcon = () => (
  <svg {...base}>
    <path d="M8 1.8v6.4M5.4 5.8 8 8.4l2.6-2.6" />
    <path d="M2.2 9.2h3l.8 1.8h4l.8-1.8h3v3.6a1 1 0 0 1-1 1H3.2a1 1 0 0 1-1-1z" />
  </svg>
);

// The app's logo: an owl's face, ear tufts, two round eyes and a beak.
export const OwlLogo = ({ size = 28 }) => (
  <svg {...base} width={size} height={size} viewBox="0 0 32 32" strokeWidth={2} className="logo">
    <path d="M6 12V4.5l5.5 4.2a12 12 0 0 1 9 0L26 4.5V12c1.6 2.2 2.4 4.700 2.400 7.400C28.400 25.500 22.800 29.500 16 29.500S3.600 25.500 3.600 19.400C3.600 16.700 4.400 14.200 6 12z" />
    <circle cx="11.500" cy="15.500" r="3.500" />
    <circle cx="20.500" cy="15.500" r="3.500" />
    <circle cx="11.500" cy="15.500" r=".8" fill="currentColor" />
    <circle cx="20.500" cy="15.500" r=".8" fill="currentColor" />
    <path d="M14.500 21.500 16 24l1.500-2.500z" fill="currentColor" />
  </svg>
);
