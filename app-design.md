# Hermes Client app design

Design reference from the project owner. Add every screenshot, colour, font, layout and
behaviour detail they provide here. Reread before and during UI work.

## Global rules

- One consistent design across the whole app unless stated otherwise per screen.
- All text uses language keys; the build shows the key name until the owner supplies the text.
- Missing assets or instructions: use a marked placeholder, don't invent a design.

## Window (v1)

- Borderless window with **rounded corners**. v1 used radius 30 ("30 degrees"); the owner then
  asked for the whole window to be **less rounded**: now **14 px** (`theme::WINDOW_RADIUS`).
  The window is clipped to that shape by Windows (window region), so nothing behind shows as
  square corners, and drawn with transparency so the **corners are smooth, not pixelated**.
- **Outlines**: 2 px, colour `#4C4C50` on every panel, control and dropdown (owner: a little
  thicker and lighter than v1's 1.5 px `#3A3A3C`).
- **Shape scale** (`theme.rs`): window 14, panels (sidebar, main panel) 12, and **every control
  (Play button, dropdowns) shares one shape: radius 10** (owner: dropdowns and the launch button
  should have the same general shape and roundedness).
- **Sidebar runs the full window height** (owner flagged it stopping short at the top and being cut
  at the bottom), and is **a little bigger than the window so it pops out**: it overhangs the main
  window by 8 px at the top, bottom and left (`theme::SIDEBAR_POP`), with all corners at the window
  radius and **no shadow** (owner asked for it removed). The logo is centred in it.
- **Custom bar at the top** of the window instead of the Windows title bar. Owner gave no layout
  for it; current interpretation: bar across the top of the main column (right of the sidebar),
  draggable, with minimise and close on the right and the `launcher.title` key on the left.
- Opens at its default (non-maximised) size and **cannot be resized or maximised**. Main window
  is **1200 px wide** (owner asked for it bigger than v1's 1000); height follows the template ratio.
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
| Profile dropdown | 796, 675, 256, 58 | **Profile selector**, left of Play (owner request, no mock-up: same size as Play, 18 gap, opens upward) |

## Control states (polish pass)

- **Play button**: ready = green with dark text; working/running = muted green (`PLAY` 60% toward
  `PANEL`) with light text, and a thin green progress track along the bottom while downloading;
  failed = outlined in `DANGER` (`#FF453A`) with `#FF8A84` text, click to retry, hover for details.
  Labels truncate with an ellipsis inside the button.
- **Hover / pressed**: white glass tints (`GLASS_HOVER`, `GLASS_PRESS`) on dropdowns, rows and window
  buttons; Play brightens on hover and darkens when pressed; close turns red.
- **Press animation**: when a button or dropdown is clicked it smoothly shrinks (to 94%) while
  held, then springs back with a slight overshoot past full size before settling (shared spring in
  `theme::press_scale`; applies to Play, window buttons and both dropdowns).
- **Keyboard focus**: 2 px `#0A84FF` ring just outside the focused control (Tab moves focus,
  Enter/Space activates).
- **Shape scale**: window 14, panels 12, controls 10, small controls 7. Every drawn icon uses a 2 px line.
- Tooltips use the app palette (panel fill, outline, control radius).

## Profiles

- Data folder: `%APPDATA%\hermes_client\` (renamed from `.hermes`; the old folder is moved once).
- Each profile is a folder `profiles\<uuid>\`, and that folder is also its Minecraft game folder
  (mods, config, saves, options). Downloaded game files (libraries, assets, Java) are shared.
- Inside it, `profile.json` holds the profile's details: `name` (optional), `icon` (file name of a
  PNG in the same folder, optional), `created` (orders the list). New details get added here.
- The selected profile is remembered in `launcher.json` in the data folder.
- The dropdown left of Play picks which profile Play launches and shows each profile's icon.
- **Creating** profiles from the launcher was removed for now: the owner will add it later.
  No design yet for creating, renaming or deleting profiles.

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
| `launcher.window.close` | Close button in the top bar (screen-reader name, not drawn) | |
| `launcher.window.minimise` | Minimise button in the top bar (screen-reader name, not drawn) | |
| `launcher.profile.unnamed` | Profile dropdown, a profile without a name | |

## Assets

- Logo: none yet. Placeholder `assets/images/logo_placeholder.png` (red square).
- Account head: none yet. Placeholder blue square; real skin heads need online accounts.
