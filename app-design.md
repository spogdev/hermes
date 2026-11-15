# Hermes Client app design

Design reference from the project owner. Add every screenshot, colour, font, layout and
behaviour detail they provide here. Reread before and during UI work.

## Global rules

- One consistent design across the whole app unless stated otherwise per screen.
- All text uses language keys; the build shows the key name until the owner supplies the text.
- Missing assets or instructions: use a marked placeholder, don't invent a design.

## Window

- Borderless, **fixed size** (can't be resized or maximised), **1200 x 707 px**, rounded corners
  **14 px** (`theme::WINDOW_RADIUS`; v1 was 30, owner asked for less rounded). Windows clips the
  window to its shape, and it is drawn with transparency so the **corners are smooth**.
  Windows never shows its own title bar, not even for a moment when the window is clicked back
  into focus (owner report; `window_shape::keep_caption_hidden`).
- **Living background** (owner request): below the top bar, five soft round gradient blobs drift
  slowly behind everything (`theme::backdrop`, colours in `BLOBS`): night colours around the ink,
  deep indigo `#262B6A` (42%), oxblood `#5A1E2A` (30%), midnight blue `#182E58` (40%), bronze
  `#5C401C` (20%) and a trace of gold (6%), so the lower background stays close to `#0E1118`. Each is small (radius 18-30% of the area's
  height) so they read as separate spots, not one glow, and wanders its own looping path (30-50 s
  per loop), its centre kept a radius in from the edges so it never looks squared off. They fade out over 40 px towards the window's
  edges and over 90 px into the plain top bar. Redrawn at 30 fps; they hold still with reduced
  motion.
- **Palette: Ink & Gold** (owner picked it in the colour revamp, replacing the cool slate and light
  blue look). After Hermes himself: near-black **ink** surfaces, **warm ivory** text and **one gold
  accent** for everything that acts or is selected. Background `#0E1118`, panels `#171C26`, dividers
  `#2A3140`, text `#F3EEE3` (16:1). **Accent gold `#E8C55F`** (owner: was `#E8B44C`, read too orange) (`theme::PLAY`; Launch, Create, Add
  Mods, Save, switches, links, ticks, focused fields, the selected outline, text selection and caret)
  with ink text `#1A1305` on it (11:1); its light partner is champagne `#F8EBC2` (`SHINE`). Stop and
  Delete are warm red `#C8382F` with white text (5.2:1); error text `#FF9A8F`. The frosted glass
  rings are ivory with a hint of gold (`FROST` `#FFF4DE`). A light
  palette from an earlier trial is kept in `theme.rs` (switching is one line, `pub use light::*`).
- **Outlines**: 2 px; the **window and sidebar** diamond is **3 px** (owner request: a little thicker; `DIAMOND_STROKE_WIDTH`). **Only the window and the sidebar** use the **diamond-like gradient** (Launch
  gold `#E8C55F` to champagne `#F8EBC2` along the diagonal with faint facet bands). **Everything
  else** (content box, dropdowns) has a **subtle solid white** (white at 43% opacity; owner
  request, was muted blue `#2F566D`; light mode keeps `#8CCFEE`), the ivory text colour at 43%, brightening to the gold when selected (owner: gradient on the sidebar and window only).
- **Animated outlines** (owner request): the window and sidebar diamond always has a soft **glint**
  drifting around it (white at up to 60%, a wide band, one lap about every 9 s; `GLINT_SPEED`,
  `GLINT_STRENGTH`). When the sidebar is hovered the glint fades into the brighter, faster
  selected band (a lap every ~3 s). Drawn at the backdrop's 30 fps; off with reduced motion.
- **Shape scale** (`theme.rs`): window 14, panels 12, **controls (Launch, dropdowns) 14**, small
  controls 9. Every drawn icon uses a 2.5 px line.

## Layout

The v1 template (below) set the arrangement; the current build lays it out directly in
`theme::body_layout` (all gaps 14 px):

- **Sidebar**: full window height, **75 px** wide (50% wider on request), flush with the window's
  left, top and bottom edges. **Logo** (the owner's white wing) 48 px at its top.
- **Top bar** (`BAR_HEIGHT` 50 px, thicker on request): right of the sidebar, draggable except over
  its controls, with a **faint divider** (1 px, `#2A3140`) along its bottom edge. From the right:
  **close**, **minimise** (40 x 32 px buttons) and the **account switcher** (210 x 36 px, moved
  into the bar on request). No title. Close and minimise show the **hand cursor** on hover (owner
  request).
- **Content box**: from below the top bar **down to the bottom of the window** (the Launch button
  was removed on request: profiles launch from their cards). On Home (and Settings, for
  now) it takes the **right half** of the space between the sidebar and the right edge, the left
  half left free; on **Profiles** it takes the **full width** (owner request).
- **Panes are Liquid Glass** (owner request, after Apple's iOS style; `theme::pane`): a
  translucent body (55% of the panel colour) that the drifting background shows through, a soft
  white sheen fading from the top (6%) to none at the bottom, and a **frosted rim** like the
  sidebar selector, made lighter and shinier on request (`Light::PANE`: base 28%, top +16%, corner glints up to 96%, inner rim 58), bright where light catches the top-left and bottom-right corners. No
  real blur (egui can't sample what's behind), so the background shows through softened only by
  the tint. It replaces the pane's solid outline.

![Template v1](design/template-v1.png)

## Home screen

The default page (Settings is empty so far). Its right pane lists profile cards (see below):
**pinned profiles always, at the top**, then **recently played** profiles (ones never launched
aren't shown); each group most recently played first (a pinned profile never launched comes last
among the pinned). With nothing pinned or played the pane is empty.

## Profiles page

Under Home in the sidebar. Its pane takes the **full width** and lists **every profile** (owner
request) as cards, oldest first. Longer lists than fit are cut off for now (no scrolling yet).
The pane starts a button's height lower (owner request) to make room for a **Create** button
(132 x 36, top right, **filled gold** with a dark "+" and `profiles.create`, like Add Mods;
owner request), which
opens the profile popup.

## Profile cards

Home and Profiles show the same cards (68 px tall, glass tint, **no outline at rest** (owner request), 14 px radius):

- **Icon** (44 px, 9 px radius, left): the profile's icon; without one, a glass tile with a **"?"**.
- **Name** in Afacad Flux Bold (or `profile.unnamed`), and **under it the client and Minecraft
  version in Afacad Flux Regular** at 65% (`profile.version`, e.g. "PvP" over "Hermes 26.1.2";
  older profiles without a recorded version show 26.1.2).
- **Launch**: hovering a card turns its icon into a **Launch button** (rounded square, gold
  `#E8C55F`, dark play triangle with well-rounded corners, owner request; the same triangle is on
  the profile page's Launch button); clicking it launches that profile. Nothing else launches games:
  the Launch button was removed (owner request). While another game is starting or running, other
  cards don't offer Launch.
- **Launching**: the card being launched gets a gold outline (fading in); its icon dims under a
  ring (filling with download progress, else turning) and the version line shows the step
  (`status.*`). **While its game runs**, the icon is a **red Stop button** (`#C8382F`, white
  square), always shown: Stop asks the game window to close (it saves and exits normally);
  pressing it again, or if the game has no window yet, ends the process. **A failed launch** shows
  `status.failed` in the error colour in place of the version (hover the card for details); hover
  to launch again.
- **Pin** (right): the owner's push-pin icon (20 px), **filled when pinned**, the outline version
  (only while the card is hovered) when not; click to pin or unpin (saved in `profile.json`).
- **Clicking anywhere else on the card opens the profile's page** (see below; owner mock-up).
  The profile editor (the popup) now opens from that page's settings button.
- Hovered cards get the **sidebar's selection box** (the lighter frosted ring over their edge,
  fading in over 0.12 s; owner request), on Home and Profiles.
- Cards darken while pressed and **shrink with everything on them** (icon, text, pin; owner
  request), though less than other buttons (owner request): about
  as many pixels as a 230 px button (~14 px across) instead of 6% of their width.
- An error that isn't about a launch (the data folder, saving a profile) shows in the error colour
  at the bottom of the pane.

## Profile page

Opened by clicking a profile card (owner mock-up, `images/10.png`). **Opens instantly** (owner
request): after the app opens, every profile's page is built in the background, **pinned profiles
first** (in home-screen order), then the rest, one at a time (the next starts once the last has
read its mods and fetched its first page of results). Modrinth search results (10 minutes) and mod
icons (the session) are shared by all pages, so later pages are quick and Modrinth isn't asked the
same thing repeatedly. Opening a built page shows it straight away and re-reads its mods folder in
the background; it opens on the installed mods. It uses the full width right of
the sidebar; the sidebar shows Profiles as the selected page, and picking any sidebar page leaves it.

- **Left column (40% of the width)**:
  - **Header**: the profile's icon (128 px, 14 px radius, the "?" tile without one), its name in
    Afacad Flux Bold 30, and `profile.version` ("Hermes 26.1.2") in Regular 17 at 70% under it.
    Under those, in one row 44 px tall with 10 px gaps: the **Launch button** (120 px wide, filled
    gold, see Right column for its states; owner request: left of the open-folder button),
    then three **round glass buttons** (44 px, solid outline, same hover / press as other glass
    buttons):
    - Icons are the owner's artwork (folder, share) and the sidebar's settings icon: **outline at
      rest, filled while hovered** (folder 22 px, share 21 px, settings 24 px).
    - **Open folder**: opens the profile's folder in the user's default file
      manager for folders (Explorer unless they've changed it).
    - **Share**: no function yet (owner request).
    - **Settings**: opens the profile editor popup.
  - **Mod pane** (liquid glass, below): describes **the hovered mod; if none is hovered, the last
    one hovered; if none has been hovered yet, the first in the table** (owner request). Its icon
    (76 px), name (Bold 21), author and download count (`mod.downloads`, e.g. 1.5M) in Regular 15 at
    70%, and **`mod.open_modrinth`** as a gold link (underlined on hover) that opens the
    mod's Modrinth page in the browser. Below, a rounded glass box (solid outline) with its
    **description**, pulled from Modrinth (the jar's own description for mods Modrinth doesn't
    know; the owner asked for Modrinth or CurseForge, but CurseForge's API needs a key, so only
    Modrinth for now).
- **Right column**:
  - Above the table, at the right: **`mods.browse`** ("Add Mods"), a 132 x 36 pill **filled gold**
    with dark "+" and label (owner request; lighter on hover, darker when pressed); it
    switches to browsing.
  - **Launch** (in the header row; `play`, play icon on the left; owner request) starts this profile, like hovering
    its card's icon. While its game starts it shows the step (`status.*`) with a turning ring,
    faded; while it runs it's a **red Stop** (`status.stop`, white square). Faded and unclickable
    while another profile's game is starting or running. After a failed launch, hovering it shows
    `status.failed.details`.
  - **Mods table** (liquid glass): column headers `mods.project`, `mods.author`, `mods.version`,
    `mods.options` (Regular 14 at 65%, over a 1 px divider; Options sits at the right, over its
    buttons), then one 50 px row per mod in
    the profile's `mods` folder, scrolling when there are more than fit:
    - **Project**: the mod's icon (32 px, rounded) and name (Bold 15).
    - **Author**, **Version** (Regular 14 at 70%).
    - **Options**, lined up on the **right** of the column (owner request): an **on/off switch** (gold when on; off renames the jar to
      `.jar.disabled`, which Fabric skips; the row fades to 45%) and **remove** (a bin: the first
      click turns it red, with `mod.remove.confirm` as its tooltip; a second click within 3 s
      deletes the jar).
    - Hovered rows get the **sidebar's selection box** (the lighter frosted ring, see Sidebar;
      fades in over 0.12 s; owner request).
    - **Fabric API is in every profile** (owner request): opening the page, creating a profile and
      launching install it if it's missing (or turn it back on). Its row has **no switch or remove**.
    - Name, author, icon and description come from Modrinth when it knows the jar (found by the
      jar's sha1), else from the jar's `fabric.mod.json`.
  - With no mods: `mods.empty` ("No content found") in the middle; a turning ring while the folder is read.
- **Browsing** (`mods.browse`): the pill becomes **`mods.installed`** (back chevron) and a **search
  field** (glass, magnifier, hint `mods.search` "Search mods") fills the rest of the row; searching
  starts 0.35 s after typing stops, or on Enter. **The first page of results (and their icons) loads
  when the page opens**, so browsing opens already filled (owner request). Under the search, **page
  buttons** (owner request: the results pane moved down to make room): previous, the page numbers
  (first, last and two either side of the current one, gaps as "..."), next; round, 32 px; the
  current page has the selection box; previous / next are faded at either end. The table lists
  Modrinth's **Fabric mods for the profile's Minecraft version** (the most downloaded with an empty
  search, 20 per page), never Fabric API (owner request). **Only mods with a Fabric build for the
  profile's Minecraft version are listed** (owner request): Modrinth's search matches loader and
  Minecraft version per project, so each result is checked for an actual Fabric version for that
  Minecraft version. Installing then reads the downloaded jar's own requirements (`depends` on
  `minecraft` and `fabricloader` in `fabric.mod.json`, which Fabric enforces at launch) and, if
  the newest build needs a newer Fabric Loader or other Minecraft, tries older builds (up to 6);
  if none fits, the error names what it needs. Columns: Project, Author,
  `mods.downloads` ("Downloads", e.g. 252M) and Options: a **round "+" install button**, a turning ring
  while installing, or a gold **tick** once it's in the profile. Installing downloads the
  newest Fabric version for that Minecraft version (hash-checked) into the profile's `mods`
  folder, **together with the mods it requires**. `mods.no_results` when nothing matches. Hovering a
  result shows it in the mod pane, like installed mods.
- Errors (reading the folder, a failed install or search) show in the error colour at the bottom
  of the table pane.
- The **bin** is drawn on whole pixels around a pixel-snapped centre (owner request: its lines were
  off-centre) and **rounded** (owner request): a lid with round ends, a handle rounded on top, a
  body with round bottom corners and two rounded bars inside; 2 px walls, bars and gaps.
- Icons for the bin, magnifier, back and tick buttons are vector placeholders until
  the owner provides icons.

## Profile popup (create / edit)

Opened by the Profiles page's Create button (creating) or by a profile page's settings button
(editing, the **profile editor**: filled in with the profile's icon, name, version and max RAM). A 580 x 352 card centred on the window, growing in on the shared spring. **Opaque, not glass**
(owner request: popups have no see-through background; a more modern look): solid `#1A1F2A`
(a step lighter than the panels; white in light mode), 22 px corners, a 1 px hairline edge (white
at 10%) and a large soft shadow (18 px down, 48 px blur). 28 px padding. Labels in Afacad Flux
Regular 13 at 60%. Fields are **recessed**: `#10141C`, 11 px corners, a 1 px hairline edge that
turns gold (1.5 px) while focused / hovered (`theme::field`). **Everything else is
darkened** (black at 43%, fading in over 0.15 s, only inside the window's shape) **and can't be
clicked**; the sidebar stays shut while it's open. Closes with its **x** button (top right, like
the title bar's) or **Esc**.

- **Icon** (left, 128 px tile, 16 px corners, a recessed field with a "+"): drop a PNG or JPEG anywhere on the window while the popup
  is open, or click the tile to browse (Windows file dialog). The centre square is kept, scaled to
  128 px, and shown in the tile; empty, it shows a "+". The tile highlights while a file is
  dragged over the window. Unreadable files show an error by the Create button.
- **Name** (right, 40 px recessed field, focused on open, up to 48 characters). Empty means
  unnamed (`profile.unnamed`, folder `Unnamed`).
- **Minecraft Version** (dropdown under Name): only `26.1.2` for now.
- **Max RAM** (owner request; full width under the icon, 18 px gap): label `create.memory` at the
  left, the value `create.memory.value` ("%s GB", e.g. "4 GB", "4.5 GB") at the right in full
  ivory, Regular 13, and a **slider** below (24 px row, `theme::SLIDER_*`): a 6 px pill track
  (glass tint, a step stronger on hover) filled gold up to a white 8 px knob (like the mod
  switches) that grows 1 px on hover / drag; the knob slides on a 0.08 s ease. Click or drag
  anywhere on it; focused, the arrow keys step it and Home / End jump to the ends (focus ring
  round the knob). Half-gigabyte steps from **1 GB up to the installed RAM** (16 GB if Windows
  can't say). New profiles start at **4 GB** (Mojang's default). Saved as `max_memory_mb` in
  `profile.json` and passed to the game as `-Xmx<N>M` in place of Mojang's `-Xmx4G`; profiles
  saved before this keep 4 GB.
- **Create** (bottom right, 140 x 42, gold, 11 px corners): makes the profile, saves its icon as
  `icon.png` in the profile folder and closes the popup. When editing, the same button is
  `edit.save`: it saves the name, version, max RAM and (if a new one was chosen) icon. The profile's folder
  keeps its original name. Errors appear to its left.
- **Delete** (editing only; bottom left; `edit.delete` "Delete"): **the same button as Save, in
  red** (`#C8382F`, white text; owner request). Asks first. The footer turns into the question `edit.delete.confirm`, a text button
  `edit.delete.cancel` and the red Delete in Save's place; Esc or Cancel backs out. Confirming moves the profile's folder (game folder: saves,
  mods, settings) to the **Windows Recycle Bin**, so it can be restored, and drops the profile from
  the lists; its page closes back to Profiles. Faded and unclickable while that profile's game is
  starting or running.

## Sidebar

- **On hover it grows like the buttons and dropdowns** (same spring): it widens over the window to
  **310 px** and grows **6 px past the window's top, bottom and left edges** (a little less than the
  8 px overhang it once had). The window keeps transparent room for that, invisible while collapsed.
- While it opens, the **logo grows a little** (48 to 54 px) and **"Hermes" appears to its right in
  Pacifico**, its letters as tall as the logo, revealed as the sidebar widens and fading in.
- **Organic motion** (owner request): everything moves on the shared spring, layered so the
  sidebar reads as soft material: the **width leads**; the **growth past the window edges and the
  logo trail it** on their own springs (a beat behind); the **leading corners round out with
  speed** (up to +26 px radius) and settle back; **"Hermes" waits until the panel is past halfway**,
  then slides in 10 px while fading up, and **leaves first when closing**. Opens in about 0.5 s.
- **Sweeping in fast from the left doesn't open it** (owner request): while collapsed, a pointer
  reaching it moving right (in from outside the window) only opens it once slower than 400 px/s (the same speed that counts as heading out of it),
  and never in its first 80 ms over the sidebar (the pointer's speed isn't known yet right after
  it enters the window).
  Coming from the right (the content) opens it at any speed.
- **Leaving the window closes it immediately** (owner request), even while it's still opening: the
  one exception to finishing a move before reversing.
- **Predictive close** (owner request): while it's open, a fast pointer move (400+ px/s) whose
  path 0.12 s ahead leaves the sidebar starts closing it immediately, before the pointer actually
  exits. Fast moves that stay inside it don't, and neither do flicks that are more vertical than
  horizontal (owner request).
- **Reduced motion**: with Windows' "Show animations" turned off, every spring (sidebar, buttons,
  dropdowns) jumps straight to its end state and the outline's light band holds still.
- **A move always finishes before it can reverse**: once it starts opening it fully opens, once it
  starts closing it fully closes. Opening overshoots slightly; closing doesn't shrink below the
  collapsed size.
- Expanded it's a glass panel with a shadow and the selected outline.
- **Navigation rows**: **Home** under the logo, **Profiles** right under Home (6 px gap) and
  **Settings** at the very bottom (owner's icons, `assets/images/icon_home*.png`,
  `icon_profiles*.png`, `icon_settings*.png`). Icons **32 px**, labels 17 px (reduced from 40/20 on
  request). Collapsed, only the icon shows, centred in the sidebar; as it opens the label appears
  beside it on the same slide-and-fade as the wordmark. **Clicking a row opens its page** (Settings
  is an empty pane until it's designed). Each icon has two versions sharing one
  frame so swaps don't shift: **outline**, and **filled** while its page is open. Hovering doesn't
  change the icon (a bold-outline hover stage was tried and removed on request); the selector shows
  hover. The outline house is cut from the filled house's shape: the roof is a ring and the walls
  are open at the top under it, like the owner's original outline icon. The filled profiles icon
  is drawn about 7% smaller in its source than the outline one, so it's scaled up to match. Rows
  show the **hand cursor** and **shrink when pressed** (icon, label and selector together) like
  every other button.
- **Held buttons stay pressed** for as long as they're held (owner request; egui otherwise drops a
  press after 0.8 s). Applies to every button.
- **Selector**: an **iOS-style frosted-glass ring** (owner request) over a faint white tint (5%;
  made lighter / whiter on request, `theme::selector`; panes keep the dimmer ring): a 4 px translucent
  cool-white edge that brightens where light catches it (top-left and bottom-right corners, top more
  than bottom), a fine bright inner rim and a faint dark outer rim for depth. Shown **only while
  hovered and the sidebar is open** (never when it's closed or closing; fades over 0.12 s). Fixed
  length: the full width of the open sidebar. The pages the rows open aren't designed yet, so
  clicking does nothing.
- **Hover / pressed**: white glass tints (`GLASS_HOVER`, `GLASS_PRESS`) on dropdowns, rows and window
  buttons; Gold buttons brighten on hover and darken when pressed; close turns red.
- **One spring for all motion** (`theme::spring`): button presses (shrink to 94%, spring back with
  about 14% overshoot), dropdowns opening and closing, and the sidebar all move on it at the same
  pace. The sidebar panel bounces a little less (about 7%), and its logo and text don't overshoot
  at all, so nothing inside it wobbles (owner request).
- **Selected outline**: when an element is selected (hovered, keyboard-focused or open; the
  sidebar while expanded) its outline eases to 3 px and a soft band of light travels slowly along
  the gradient (`theme::selectable_outline`).
- **Keyboard focus**: 2 px light gold `#FFE39A` ring just outside the focused control (Tab moves focus,
  Enter/Space activates).
- **Cursor**: the pointer (hand) cursor stays on for the whole time the mouse is over a dropdown,
  in every state, with no flicker back to the arrow (owner request).
- Tooltips use the app palette (panel fill, outline, control radius).

## Text

- **All text in Afacad Flux Bold, 15% larger** than before (`theme::TEXT_SCALE` = 1.15, applied to
  every label and tooltip), except secondary lines (a card's version) in **Afacad Flux Regular**
  (`theme::regular_font`, owner request: "non bold").
- **"Hermes" in the sidebar uses Pacifico** (only there).

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
  PNG in the same folder, optional), `created` (orders the list), `pinned` (optional: listed on the home screen), `minecraft`
  (version chosen at creation), `last_played` (last launch; 0 or missing = never). New details get added here.
- The selected profile is remembered in `launcher.json` in the data folder.
- Each card launches its own profile; the last one launched is remembered in `launcher.json`, and
  each profile's `last_played` (in `profile.json`) orders the home screen.
- **Creating** profiles: the create-profile popup (see above). **Deleting**: the profile editor's
  Delete (to the Recycle Bin). No design yet for renaming profile folders.
- **Mods** live in the profile folder's `mods` folder (Fabric's usual place); turned-off mods are
  renamed `*.jar.disabled`. The Hermes client mod isn't in there (it's added at launch).

## Dropdowns

- Dropdowns (the account switcher) **expand in place into the list of options**.
- The **dropdown arrow morphs into a check mark** that marks the selected option.
- Look and motion **inspired by Apple iOS "Liquid Glass"**: translucent glass panel, soft light
  edge, springy expand. (egui can't blur what is behind a window region, so the glass is
  approximated with translucency, highlight and shadow.)

## Language keys

Owner fills in the text after reviewing where each key appears in the build.
Files: `assets/lang/<code>.json` (Minecraft Java style). Empty value = the key itself is shown.
Keys have no `launcher.` prefix (owner request).

| Key | Where it appears | Text |
|---|---|---|
| `play` | Profile card's Launch button (screen-reader name), and the profile page's Launch button | Launch |
| `status.metadata` | Card of the profile being launched, under its name: reading game metadata | |
| `status.checking` | Same, checking game files | Checking... |
| `status.downloading` | Same, downloading (`%1$s` done, `%2$s` total) | |
| `status.preparing_java` | Same, building the Java class cache (first launch) | |
| `status.starting` | Same, starting Minecraft | |
| `status.stop` | Card's red Stop button while its game runs (screen-reader name), and the profile page's Stop button | Stop |
| `status.failed` | Card of a profile whose launch failed, under its name | |
| `status.failed.details` | Tooltip when hovering that card (`%s` = technical error) | |
| `profile.pin` | Card's pin button (screen-reader name, not drawn) | |
| `edit.save` | Profile editor: the Save button (in place of Create) | Save |
| `edit.delete` | Profile editor: Delete (bottom left), and the red button that confirms it | Delete |
| `edit.delete.confirm` | Profile editor: the question asked before deleting | |
| `edit.delete.cancel` | Profile editor: backs out of deleting | |
| `window.close` | Close button in the top bar (screen-reader name, not drawn) | |
| `window.minimise` | Minimise button in the top bar (screen-reader name, not drawn) | |
| `profile.unnamed` | Profile cards, a profile without a name | Unnamed Profile |
| `profile.version` | Profile cards, under the name (`%s` = Minecraft version) | Hermes %s |
| `sidebar.home` | Sidebar Home row label (shown when the sidebar is open) | Home |
| `sidebar.profiles` | Sidebar Profiles row label (shown when the sidebar is open) | Profiles |
| `profiles.create` | Profiles page Create button (next to its "+") | Create |
| `create.icon` | Create-profile popup: label over the icon tile | Icon |
| `create.name` | Create-profile popup: label over the name field | Name |
| `create.version` | Create-profile popup: label over the version dropdown | Minecraft Version |
| `create.memory` | Profile popup: label over the max-RAM slider | Memory |
| `create.memory.value` | Profile popup: the slider's value, right of its label (`%s` = gigabytes, e.g. 4 or 4.5) | %s GB |
| `create.confirm` | Create-profile popup: the Create button | Create |
| `create.close` | Create-profile popup: close button (screen-reader name, not drawn) | |
| `sidebar.settings` | Sidebar Settings row label (shown when the sidebar is open) | Settings |
| `profile.open_folder` | Profile page: open-folder button (screen-reader name, not drawn) | |
| `profile.share` | Profile page: share button (screen-reader name, not drawn) | |
| `profile.settings` | Profile page: settings button (screen-reader name, not drawn) | |
| `mods.project` | Mods table: first column header | Project |
| `mods.author` | Mods table: second column header | Author |
| `mods.version` | Mods table: third column header (installed mods) | Version |
| `mods.options` | Mods table: last column header | Options |
| `mods.downloads` | Mods table while browsing: third column header | Downloads |
| `mods.browse` | Profile page: pill above the table that starts browsing Modrinth | Add Mods |
| `mods.installed` | Same pill while browsing: back to the installed mods | Back |
| `mods.search` | Search field hint while browsing (also its screen-reader name) | Search mods |
| `mods.empty` | Mods table: the profile has no mods | No content found |
| `mods.no_results` | Mods table while browsing: nothing found | |
| `mod.downloads` | Download count (`%s` = short number like 1.5M): mod pane and browse table | %s |
| `mod.open_modrinth` | Mod pane: link to the mod's Modrinth page | Open in Modrinth |
| `mod.enable` | Mods table: on/off switch (screen-reader name, not drawn) | |
| `mod.remove` | Mods table: remove button (screen-reader name, not drawn) | |
| `mod.remove.confirm` | Tooltip on the remove button after one click (click again to remove) | |
| `mod.install` | Browse table: install button (screen-reader name, not drawn) | |
| `mods.page.previous` | Result pages: previous-page button (screen-reader name, not drawn) | |
| `mods.page.next` | Result pages: next-page button (screen-reader name, not drawn) | |

## Assets

- **Fonts** (both SIL Open Font License; the licence files must ship with them):
  - **Afacad Flux Bold** (`assets/fonts/AfacadFlux-Bold.ttf`, `OFL-AfacadFlux.txt`): all text.
  - **Afacad Flux Regular** (`assets/fonts/AfacadFlux-Regular.ttf`, same licence, from the owner's
    Afacad Flux download): secondary lines.
  - **Pacifico** (`assets/fonts/Pacifico-Regular.ttf`, `OFL-Pacifico.txt`): the sidebar's "Hermes".
  - egui's bundled fonts were removed. Afacad Flux covers Latin scripts only: a language in
    Cyrillic, Greek, CJK or Arabic needs a fallback font for that script.
- **Logo**: the owner's white wing on transparent (`assets/images/logo.png`, cropped and resized to
  256 px from the supplied 1024 px image; drawn with mipmaps so it stays smooth when small).
- **App icon** (taskbar, window, Explorer): **just the white wing on transparent** (owner request;
  the dark blue circle behind it was removed), 240 px wide at 256 px, centred left to right and
  raised to the top edge (owner request: a little higher on the taskbar). `assets/images/app_icon.png`
  sets the window/taskbar icon; `app_icon.ico` (16-256 px) is embedded in the exe by `build.rs`.
- **Pin icons** (owner artwork): `icon_pin.png` (outline) / `icon_pin_filled.png`, cropped to one
  shared box, squared, 128 px.
- **Profile page header icons** (owner artwork, white on transparent): `icon_folder.png` /
  `icon_folder_filled.png`, `icon_share.png` / `icon_share_filled.png`. Each pair is cropped to
  the same box (so hovering doesn't shift them), squared and resized to 128 px.
- Account head: none yet. Placeholder gold square; real skin heads need online accounts.
