# Hermes Client app design

Design reference from the project owner. Add every screenshot, colour, font, layout and
behaviour detail they provide here. Reread before and during UI work.

## Global rules

- One consistent design across the whole app unless stated otherwise per screen.
- All text uses language keys; the build shows the key name until the owner supplies the text.
- Missing assets or instructions: use a marked placeholder, don't invent a design.

## Window (v1)

- Borderless window with **rounded corners, radius 30** (owner said "30 degrees"; implemented as a
  30 px corner radius).
- **Custom bar at the top** of the window instead of the Windows title bar. Owner gave no layout
  for it; current interpretation: full-width bar above the template layout, draggable, with
  minimise and close buttons on the right and the `launcher.title` key on the left.
- Opens at its default (non-maximised) size and **cannot be resized or maximised**.
- **Typical dark-mode colours** for now (iOS dark palette: background `#1C1C1E`,
  panels `#2C2C2E`, outlines `#3A3A3C`, text `#F2F2F7`).

## Main screen template (v1)

![Template v1](design/template-v1.png)

Layout, measured on the template's outer frame (1340 x 749) and scaled to the window:

| Element | Template rect (x, y, w, h) | Meaning |
|---|---|---|
| Sidebar | 0, 0, 66, 749 | Full-height rounded column on the left edge |
| Red box | 10, 12, 46, 40 | **Logo** (placeholder image `assets/images/logo_placeholder.png`) |
| Account pill | 1095, 13, 231, 50 | **Account selector**: head, account name, dropdown arrow |
| Blue box | inside the pill, left | **Head of the selected Minecraft account** (placeholder) |
| Large panel | 78, 78, 1248, 579 | Main content area (contents not specified yet) |
| Green button | 1070, 675, 256, 58 | **Play button** |

## Dropdowns

- Dropdowns (e.g. the account selector) **expand in place into the list of options**.
- The **dropdown arrow morphs into a check mark** that marks the selected option.
- Look and motion **inspired by Apple iOS "Liquid Glass"**: translucent glass panel, soft light
  edge, smooth expand animation. (egui can't blur what is behind a window region, so the glass is
  approximated with translucency, highlight and shadow.)

## Language keys

Owner fills in the text after reviewing where each key appears in the build.
Files: `assets/lang/<code>.json` (Minecraft Java style). Empty value = the key itself is shown.

| Key | Where it appears | Text |
|---|---|---|
| `launcher.title` | Custom top bar, left | |
| `launcher.play` | Play button, idle | |
| `launcher.status.metadata` | Play button while reading game metadata | |
| `launcher.status.checking` | Play button while checking game files | |
| `launcher.status.downloading` | Play button while downloading (`%1$s` done, `%2$s` total) | |
| `launcher.status.preparing_java` | Play button while building the Java class cache (first launch) | |
| `launcher.status.starting` | Play button while starting Minecraft | |
| `launcher.status.running` | Play button while the game is running | |
| `launcher.status.failed` | Play button after a failed launch | |
| `launcher.status.failed.details` | Tooltip when hovering a failed Play button (`%s` = technical error) | |

## Assets

- Logo: none yet. Placeholder `assets/images/logo_placeholder.png` (red square).
- Account head: none yet. Placeholder blue square; real skin heads need online accounts.
