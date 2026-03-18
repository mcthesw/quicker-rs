# quicker-rs

This build keeps the radial menu inside the egui window only.

- Right-drag inside the Quicker window opens the radial menu.
- Global right-drag is intentionally disabled.
- There is no KDE/KWin effect integration in this tree anymore.

On Wayland, this avoids the unstable global-overlay path and keeps mouse handling on the Rust side only.
