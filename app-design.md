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
- **Outlines**: 2 px on every panel, control and dropdown, drawn as a **diamond-like gradient**: from
  the Play blue `#5AC8FA` to light blue `#D6F2FF` along the diagonal with faint facet bands
  (`theme::outline`). (Before: flat `#4C4C50`.)
- **Shape scale** (`theme.rs`): window 14, panels (sidebar, main panel) 12, and **every control
  (Play button, dropdowns) shares one shape: radius 14** (was 10; owner asked for a little more curve) (owner: dropdowns and the launch button
  should have the same general shape and roundedness).
- **Sidebar runs the full window height** (owner flagged it stopping short at the top and being cut
  at the bottom), and is **a little bigger than the window so it pops out**: it overhangs the main
  window by 8 px at the top, bottom and left (`theme::SIDEBAR_POP`), with all corners at the window
  radius and **no shadow** (owner asked for it removed). The logo is centred in it.
  **On hover it expands like the dropdowns**: grows to 220 px wide over the window as a glass panel
  with a shadow, on the shared spring (it overshoots its full width slightly, like a press bounce;
  closing doesn't bounce narrower, which would uncover the window edge); collapses when the pointer
  leaves. Its contents beyond the
  logo aren't designed yet. **A move always finishes before it can reverse**: once it starts
  opening it fully opens (even if the pointer leaves), and once it starts closing it fully closes.
- **Custom bar at the top** of the window instead of the Windows title bar: across the top of the
  main column (right of the sidebar), draggable, minimise and close on the right, and the
  **"Hermes" wordmark** on the left in the font **Amsterdam Four** (by Lettersiro, 32 px,
  set low in the bar (moved down again on request) and inset 22 px; owner request, replaces the old title key). The wordmark
  is brand text, not translated. **Licence: the font file is "free for personal use" (preview
  version)**, so it lives only on the owner's machine at `assets/fonts/amsterdam_four.ttf`, is
  git-ignored, and is embedded only when present (`build.rs`); builds without it fall back to the
  UI font. Distributing the launcher with it needs the full licence (Creative Fabrica).
- Opens at its default (non-maximised) size and **cannot be resized or maximised**. Main window
  is **1200 px wide** (owner asked for it bigger than v1's 1000); height follows the template ratio.
- **Dark theme** (back after a temporary light trial; the light palette is kept in `theme.rs`): (iOS dark palette: background `#1C1C1E`,
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

- **Play button**: **light blue** `#5AC8FA` with dark text; working = muted blue (60% toward `PANEL`)
  with light text, and a thin blue progress track along the bottom while downloading;
  **while the game runs it becomes a red Stop button** (`#D7263D`, white text): Stop asks the game
  window to close (it saves and exits normally); pressing it again, or if the game has no window
  yet, ends the process;
  failed = outlined in `DANGER` (`#FF453A`) with `#FF8A84` text, click to retry, hover for details.
  Labels truncate with an ellipsis inside the button.
- **Hover / pressed**: white glass tints (`GLASS_HOVER`, `GLASS_PRESS`) on dropdowns, rows and window
  buttons; Play brightens on hover and darkens when pressed; close turns red.
- **Press animation**: when a button or dropdown is clicked it smoothly shrinks (to 94%) while
  held, then springs back with a small overshoot (about 14% of the distance; reduced from about
  21% on request) before settling. Applies to Play, window buttons and both dropdowns.
- **One spring for everything that bounces** (`theme::spring`): press feedback and the sidebar use
  the same spring, so they bounce alike.
- **Keyboard focus**: 2 px `#0A84FF` ring just outside the focused control (Tab moves focus,
  Enter/Space activates).
- **Shape scale**: window 14, panels 12, controls 14, small controls 9. Every drawn icon uses a 2 px line.
- Tooltips use the app palette (panel fill, outline, control radius).

## Profiles

- Data folder: `%APPDATA%\HermesClient\` (earlier builds used `hermes_client` and `.hermes`; the
  newest of those is moved over once on first run).
- Each profile is a folder `profiles\<name>\`, and that folder is also its Minecraft game folder
  (mods, config, saves, options). Downloaded game files (libraries, assets, Java) are shared.
- **Folder names come from the profile's name when it is created** (owner request; no ids):
  no name -> `Unnamed`; if taken -> `Unnamed (1)`, `Unnamed (2)`, ... using the lowest free number
  above 0. Named profiles work the same way (`PvP`, `PvP (1)`). Characters Windows forbids become
  `_`. The folder name stays fixed afterwards (renaming isn't designed yet).
- Inside it, `profile.json` holds the profile's details: `name` (optional), `icon` (file name of a
  PNG in the same folder, optional), `created` (orders the list). New details get added here.
- The selected profile is remembered in `launcher.json` in the data folder.
- The dropdown left of Play picks which profile Play launches and shows each profile's icon.
- **Creating** profiles from the launcher was removed for now: the owner will add it later, with
  the name picked by the user. No design yet for creating, renaming or deleting profiles.

## Dropdowns

- Dropdowns (e.g. the account selector) **expand in place into the list of options**.
- The **dropdown arrow morphs into a check mark** that marks the selected option.
- Look and motion **inspired by Apple iOS "Liquid Glass"**: translucent glass panel, soft light
  edge, smooth expand animation. (egui can't blur what is behind a window region, so the glass is
  approximated with translucency, highlight and shadow.)

## Language keys

Owner fills in the text after reviewing where each key appears in the build.
Files: `assets/lang/<code>.json` (Minecraft Java style). Empty value = the key itself is shown.
Keys have no `launcher.` prefix (owner request).

| Key | Where it appears | Text |
|---|---|---|
| `play` | Play button, idle | Launch |
| `status.metadata` | Play button while reading game metadata | |
| `status.checking` | Play button while checking game files | Checking... |
| `status.downloading` | Play button while downloading (`%1$s` done, `%2$s` total) | |
| `status.preparing_java` | Play button while building the Java class cache (first launch) | |
| `status.starting` | Play button while starting Minecraft | |
| `status.stop` | Red Stop button shown in place of Play while the game runs; closes the game | Stop |
| `status.failed` | Play button after a failed launch | |
| `status.failed.details` | Tooltip when hovering a failed Play button (`%s` = technical error) | |
| `window.close` | Close button in the top bar (screen-reader name, not drawn) | |
| `window.minimise` | Minimise button in the top bar (screen-reader name, not drawn) | |
| `profile.unnamed` | Profile dropdown, a profile without a name | Unnamed Profile |

## Assets

- Wordmark font: Amsterdam Four (local file only, see Window section).
- Logo: none yet. Placeholder `assets/images/logo_placeholder.png` (red square).
- Account head: none yet. Placeholder blue square; real skin heads need online accounts.
