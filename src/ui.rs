//! Main launcher window (design v1, see `app-design.md`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{
    self, Align2, Context, CursorIcon, Id, Rect, Sense, Stroke, TextureHandle, TextureOptions,
    ViewportCommand, pos2, vec2,
};

use crate::profile_list;
use crate::profile_page::{self, ProfilePage};
use crate::profile_popup::{self, ProfilePopup};
use crate::dropdown;
use crate::lang::{tr, tr_args};
use crate::launch::{self, Step};
use crate::profiles::Profiles;
use crate::theme;
use crate::{game_window, window_shape};

/// Developer options for visual checks (`--screenshot <png>`, `--open-dropdown account`,
/// `--state downloading|checking|running|failed` (shown on the first profile's card), `--focus account`).
#[derive(Default)]
pub struct DevOptions {
    pub screenshot: Option<PathBuf>,
    pub open_dropdown: Option<String>,
    pub state: Option<String>,
    pub focus: Option<String>,
    pub open_sidebar: bool,
    /// Page to open (`home`, `profiles`, `settings`).
    pub page: Option<String>,
    /// Open the create-profile popup.
    pub open_create: bool,
    /// Open the profile editor for the first profile.
    pub open_edit: bool,
    /// With `--page profile`: browse Modrinth.
    pub browse: bool,
    /// Frame to capture with `--screenshot` (default 45, once animations have settled).
    pub screenshot_frame: Option<u32>,
}

enum Phase {
    Idle,
    Working(Option<Step>),
    /// The game is running. `stopping`: the player asked it to close.
    Running { child: Child, stopping: bool },
    Failed(String),
}

/// The screen shown in the main pane, picked from the sidebar.
#[derive(Clone, Copy, PartialEq)]
enum Page {
    Home,
    /// Every profile, and creating new ones.
    Profiles,
    /// Not designed yet: an empty pane.
    Settings,
    /// One profile's page (`HermesApp::open_profile`), opened from its card.
    Profile,
}

impl Page {
    /// The sidebar row that shows as selected: a profile's page belongs to Profiles.
    fn nav(self) -> Page {
        if self == Page::Profile { Page::Profiles } else { self }
    }
}

struct HermesApp {
    phase: Arc<Mutex<Phase>>,
    page: Page,
    /// The profile popup (creating or editing), while it's open.
    popup: Option<ProfilePopup>,
    /// Profile pages, kept once built: every profile's page is built in the
    /// background after the app opens (pinned first), so opening one is instant.
    profile_pages: HashMap<String, ProfilePage>,
    /// Profiles whose pages are still to be built, in order.
    preload: std::collections::VecDeque<String>,
    /// The profile whose page `Page::Profile` shows.
    open_profile: Option<String>,
    /// The profile being launched or played (its card shows the progress).
    active: Option<String>,
    /// An error that isn't about a launch, shown under the profile list.
    notice: Option<String>,
    logo: TextureHandle,
    nav_icons: NavIcons,
    /// A profile page's header button icons.
    page_icons: profile_page::Icons,
    /// The profile cards' pin: outline and filled.
    pin_icons: [TextureHandle; 2],
    accounts: Vec<String>,
    selected_account: usize,
    /// Data folder; `None` if it couldn't be created (the error is shown as `notice`).
    root: Option<PathBuf>,
    profiles: Profiles,
    /// Loaded profile icons by profile id (profiles without a readable PNG have none).
    profile_icons: HashMap<String, TextureHandle>,
    /// Physical window size the rounded window region was last built for.
    region_size: Option<[i32; 2]>,
    /// Size (points) of the drawing area Windows actually shows; see `window_shape`.
    visible: Option<egui::Vec2>,
    screenshot: Option<PathBuf>,
    frames: u32,
    /// Developer focus target, applied once the widgets exist (areas skip their first frame).
    focus: Option<Id>,
    /// Developer option: keep the sidebar expanded.
    force_sidebar: bool,
    screenshot_frame: u32,
}

const ACCOUNT_DROPDOWN: &str = "account_dropdown";

impl HermesApp {
    fn new(ctx: &Context, dev: DevOptions) -> Self {
        match dev.open_dropdown.as_deref() {
            Some("account") => dropdown::set_open(ctx, Id::unique(ACCOUNT_DROPDOWN), true),
            _ => {}
        }
        theme::apply_style(ctx);
        theme::install_fonts(ctx);
        let loaded = launch::Paths::new().and_then(|paths| Ok((Profiles::load(&paths.root)?, paths.root)));
        let (profiles, root, notice) = match loaded {
            Ok((profiles, root)) => (profiles, Some(root), None),
            // No data folder: no profiles to show, and the error shows instead.
            Err(e) => (Profiles { selected: String::new(), profiles: Vec::new() }, None, Some(format!("{e:#}"))),
        };
        let mut phase = Phase::Idle;
        match dev.state.as_deref() {
            Some("downloading") => phase = Phase::Working(Some(Step::Downloading { done: 2100, total: 5247 })),
            Some("checking") => phase = Phase::Working(Some(Step::Checking)),
            Some("failed") => phase = Phase::Failed("example error".into()),
            // A harmless short-lived process stands in for the game.
            Some("running") => {
                if let Ok(child) = std::process::Command::new("ping").args(["-n", "20", "127.0.0.1"]).stdout(std::process::Stdio::null()).spawn() {
                    phase = Phase::Running { child, stopping: false };
                }
            }
            _ => {}
        }
        let active = dev.state.is_some().then(|| profiles.profiles.first().map(|p| p.id.clone())).flatten();
        let focus = match dev.focus.as_deref() {
            Some("account") => Some(dropdown::pill_id(Id::unique(ACCOUNT_DROPDOWN))),
            _ => None,
        };
        let profile_icons = root.as_deref().map(|r| load_profile_icons(ctx, r, &profiles)).unwrap_or_default();
        let popup = if dev.open_edit {
            profiles.profiles.first().map(|p| ProfilePopup::edit(p, profile_icons.get(&p.id)))
        } else {
            dev.open_create.then(ProfilePopup::create)
        };
        let mut profile_pages = HashMap::new();
        let open_profile = match (dev.page.as_deref(), &root, profiles.profiles.first()) {
            (Some("profile"), Some(root), Some(first)) => {
                let mut page = ProfilePage::open(ctx, root, first);
                if dev.browse {
                    page.browse();
                }
                profile_pages.insert(first.id.clone(), page);
                Some(first.id.clone())
            }
            _ => None,
        };
        // Pages to build in the background: pinned profiles first (in home-screen
        // order), then the rest.
        let mut preload: std::collections::VecDeque<String> =
            profiles.home_order().into_iter().filter(|p| p.info.pinned).map(|p| p.id.clone()).collect();
        for p in &profiles.profiles {
            if !preload.contains(&p.id) {
                preload.push_back(p.id.clone());
            }
        }
        HermesApp {
            force_sidebar: dev.open_sidebar,
            screenshot_frame: dev.screenshot_frame.unwrap_or(45),
            focus,
            profile_icons,
            phase: Arc::new(Mutex::new(phase)),
            page: match dev.page.as_deref() {
                Some("profiles") => Page::Profiles,
                Some("settings") => Page::Settings,
                Some("profile") if open_profile.is_some() => Page::Profile,
                _ => Page::Home,
            },
            profile_pages,
            preload,
            open_profile,
            popup,
            active,
            notice,
            logo: load_png(ctx, "logo", include_bytes!("../assets/images/logo.png")),
            pin_icons: [
                load_png(ctx, "icon_pin", include_bytes!("../assets/images/icon_pin.png")),
                load_png(ctx, "icon_pin_filled", include_bytes!("../assets/images/icon_pin_filled.png")),
            ],
            page_icons: profile_page::Icons {
                folder: [
                    load_png(ctx, "icon_folder", include_bytes!("../assets/images/icon_folder.png")),
                    load_png(ctx, "icon_folder_filled", include_bytes!("../assets/images/icon_folder_filled.png")),
                ],
                share: [
                    load_png(ctx, "icon_share", include_bytes!("../assets/images/icon_share.png")),
                    load_png(ctx, "icon_share_filled", include_bytes!("../assets/images/icon_share_filled.png")),
                ],
                settings: [
                    load_png(ctx, "icon_settings", include_bytes!("../assets/images/icon_settings.png")),
                    load_png(ctx, "icon_settings_filled", include_bytes!("../assets/images/icon_settings_filled.png")),
                ],
            },
            nav_icons: NavIcons {
                home: load_png(ctx, "icon_home", include_bytes!("../assets/images/icon_home.png")),
                home_filled: load_png(ctx, "icon_home_filled", include_bytes!("../assets/images/icon_home_filled.png")),
                profiles: load_png(ctx, "icon_profiles", include_bytes!("../assets/images/icon_profiles.png")),
                profiles_filled: load_png(ctx, "icon_profiles_filled", include_bytes!("../assets/images/icon_profiles_filled.png")),
                settings: load_png(ctx, "icon_settings", include_bytes!("../assets/images/icon_settings.png")),
                settings_filled: load_png(ctx, "icon_settings_filled", include_bytes!("../assets/images/icon_settings_filled.png")),
            },
            // Offline mode has one account; Microsoft accounts will fill this list.
            accounts: vec![launch::PLAYER_NAME.to_string()],
            selected_account: 0,
            root,
            profiles,
            region_size: None,
            visible: None,
            screenshot: dev.screenshot,
            frames: 0,
        }
    }

    /// Shows an error that isn't about a launch (saving a profile, the data folder).
    fn report(&mut self, result: anyhow::Result<()>) {
        if let Err(e) = result {
            self.notice = Some(format!("{e:#}"));
        }
    }

    /// What a profile's card shows about launching: only the profile being
    /// launched or played shows progress, Stop or an error.
    fn card_status(&self, id: &str) -> profile_list::Status {
        use profile_list::Status;
        let phase = self.phase.lock().unwrap();
        if self.active.as_deref() != Some(id) {
            return Status::Idle { can_launch: matches!(*phase, Phase::Idle | Phase::Failed(_)) };
        }
        match &*phase {
            Phase::Idle => Status::Idle { can_launch: true },
            Phase::Working(step) => {
                let (text, progress) = match step {
                    None | Some(Step::Metadata) => (tr("status.metadata"), None),
                    Some(Step::Checking) => (tr("status.checking"), None),
                    Some(Step::Downloading { done, total }) => {
                        (tr_args("status.downloading", &[&done.to_string(), &total.to_string()]), Some(*done as f32 / (*total).max(1) as f32))
                    }
                    Some(Step::PreparingJava) => (tr("status.preparing_java"), None),
                    Some(Step::Starting | Step::JavaCacheSkipped(_)) => (tr("status.starting"), None),
                };
                Status::Working { text, progress }
            }
            Phase::Running { .. } => Status::Running,
            Phase::Failed(e) => Status::Failed(e.clone()),
        }
    }

    /// The profile cards in `pane`: on Home the pinned and recently played
    /// profiles, on the Profiles page all of them. Any error not about a launch
    /// shows under the list.
    fn profile_cards(&mut self, ui: &mut egui::Ui, pane: Rect, home: bool) {
        let shown: Vec<&crate::profiles::Profile> = if home { self.profiles.home_order() } else { self.profiles.profiles.iter().collect() };
        let cards: Vec<profile_list::Card> = shown
            .into_iter()
            .map(|profile| profile_list::Card { profile, icon: self.profile_icons.get(&profile.id), status: self.card_status(&profile.id) })
            .collect();
        let picked = profile_list::show(ui, pane, if home { "home_card" } else { "profiles_card" }, &cards, &self.pin_icons);
        drop(cards);
        if let Some(notice) = &self.notice {
            let mut job = egui::text::LayoutJob::simple(notice.clone(), theme::regular_font(13.0), theme::DANGER_TEXT, pane.width() - 2.0 * theme::GAP);
            job.wrap.max_rows = 2;
            let painter = ui.painter();
            let galley = painter.layout_job(job);
            painter.galley(pos2(pane.min.x + theme::GAP, pane.max.y - theme::GAP - galley.size().y), galley, theme::DANGER_TEXT);
        }
        let Some((id, action)) = picked else { return };
        match action {
            profile_list::Action::Launch => self.start(ui.ctx(), &id),
            profile_list::Action::Stop => self.stop_game(),
            profile_list::Action::Open => {
                if let (Some(root), Some(profile)) = (&self.root, self.profiles.profiles.iter().find(|p| p.id == id)) {
                    match self.profile_pages.get_mut(&id) {
                        // Already built: shown at once, and re-read in the background.
                        Some(page) => page.reopen(),
                        None => {
                            self.profile_pages.insert(id.clone(), ProfilePage::open(ui.ctx(), root, profile));
                        }
                    }
                    self.open_profile = Some(id);
                    self.page = Page::Profile;
                }
            }
            profile_list::Action::TogglePin => {
                let result = match &self.root {
                    Some(root) => self.profiles.update(root, &id, |info| info.pinned = !info.pinned),
                    None => Ok(()),
                };
                self.report(result);
            }
        }
    }

    /// Builds the next waiting profile page, one at a time: the next starts once
    /// every built page has finished loading, so pages share what the first
    /// fetched (search results, icons) instead of all asking Modrinth at once.
    fn preload_pages(&mut self, ctx: &Context) {
        let Some(root) = self.root.clone() else { return };
        if self.preload.is_empty() || self.profile_pages.values().any(ProfilePage::loading) {
            return;
        }
        while let Some(id) = self.preload.pop_front() {
            if self.profile_pages.contains_key(&id) {
                continue;
            }
            if let Some(profile) = self.profiles.profiles.iter().find(|p| p.id == id) {
                self.profile_pages.insert(id, ProfilePage::open(ctx, &root, profile));
                break;
            }
        }
    }

    /// The open profile's page; its settings button opens the profile popup.
    fn profile_page_ui(&mut self, ui: &mut egui::Ui, area: Rect) {
        let Some(id) = self.open_profile.clone() else { return };
        let status = self.card_status(&id);
        let Some(page) = self.profile_pages.get_mut(&id) else { return };
        let Some(profile) = self.profiles.profiles.iter().find(|p| p.id == id) else { return };
        let icon = self.profile_icons.get(&profile.id);
        match page.show(ui, area, profile, icon, &self.page_icons, &status) {
            profile_page::Action::None => {}
            profile_page::Action::Settings => self.popup = Some(ProfilePopup::edit(profile, icon)),
            profile_page::Action::Launch => self.start(ui.ctx(), &id),
            profile_page::Action::Stop => self.stop_game(),
        }
    }

    /// Shows the profile popup while it's open and acts on its buttons:
    /// creates the profile, or saves the edited one.
    fn profile_popup(&mut self, ctx: &Context, window: Rect) {
        // A profile whose game is starting or running can't be deleted (its folder is in use).
        let in_use = self.popup.as_ref().and_then(|p| p.editing.clone()).is_some_and(|id| {
            self.active.as_deref() == Some(id.as_str()) && matches!(*self.phase.lock().unwrap(), Phase::Working(_) | Phase::Running { .. })
        });
        let Some(popup) = &mut self.popup else { return };
        popup.can_delete = !in_use;
        match popup.show(ctx, window) {
            profile_popup::Action::None => {}
            profile_popup::Action::Close => self.popup = None,
            profile_popup::Action::Delete => {
                let (Some(root), Some(id)) = (self.root.clone(), popup.editing.clone()) else { return };
                match self.profiles.remove(&root, &id, crate::shell::recycle) {
                    Ok(()) => {
                        self.popup = None;
                        self.profile_icons.remove(&id);
                        if self.active.as_deref() == Some(id.as_str()) {
                            self.active = None;
                        }
                        // Its page goes; if it was open, back to the list.
                        self.profile_pages.remove(&id);
                        if self.open_profile.as_deref() == Some(id.as_str()) {
                            self.open_profile = None;
                            if self.page == Page::Profile {
                                self.page = Page::Profiles;
                            }
                        }
                    }
                    Err(e) => popup.error = Some(format!("{e:#}")),
                }
            }
            profile_popup::Action::Save => {
                let Some(root) = self.root.clone() else { return };
                let request = popup.request();
                let icon = request.icon.cloned();
                let request_mc = request.minecraft.to_string();
                let saved = match popup.editing.clone() {
                    None => self.profiles.add(&root, request.name, request.minecraft, request.max_memory_mb, icon.as_ref()),
                    Some(id) => {
                        let minecraft = request.minecraft.to_string();
                        let name = request.name;
                        let memory = request.max_memory_mb;
                        self.profiles
                            .update(&root, &id, |info| {
                                info.name = name;
                                info.minecraft = Some(minecraft);
                                info.max_memory_mb = Some(memory);
                            })
                            .and_then(|()| match &icon {
                                Some(icon) => self.profiles.save_icon(&root, &id, icon),
                                None => Ok(()),
                            })
                            .map(|()| id)
                    }
                };
                let created = popup.editing.is_none();
                match saved {
                    Ok(id) => {
                        if created {
                            self.preload.push_back(id.clone());
                            // Every profile has Fabric API (owner request): fetched now
                            // in the background (launching also makes sure).
                            let (dir, mc) = (Profiles::dir(&root, &id), request_mc);
                            crate::mods::runtime().spawn(async move {
                                let target = crate::mods::target(&mc).await?;
                                crate::mods::ensure_fabric_api(&dir, &target).await
                            });
                        }
                        if let Some(img) = icon {
                            let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                            let options = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
                            self.profile_icons.insert(id.clone(), ctx.load_texture(format!("profile-icon-{id}"), color, options));
                        }
                        self.popup = None;
                    }
                    Err(e) => {
                        if let Some(popup) = &mut self.popup {
                            popup.error = Some(format!("{e:#}"));
                        }
                    }
                }
            }
        }
    }

    /// Clips the window to the window outline plus everything the hovered sidebar
    /// can grow into (rebuilt if the physical size changes, e.g. DPI) and returns
    /// the visible window area to lay the UI out in. Areas the collapsed sidebar
    /// doesn't cover are fully transparent.
    fn clip_window(&mut self, ctx: &Context, frame: &eframe::Frame) -> Rect {
        let ppp = ctx.pixels_per_point();
        let content = ctx.content_rect();
        let size = content.size() * ppp;
        let size = [size.x.round() as i32, size.y.round() as i32];
        if self.region_size != Some(size) {
            // The clip sits in the transparent edge margin around the painted shapes.
            let radius = ((theme::WINDOW_RADIUS as f32 + theme::EDGE_MARGIN) * ppp).round() as i32;
            let to_px = |r: Rect| window_shape::Shape {
                x: ((r.min.x - content.min.x) * ppp).round() as i32,
                y: ((r.min.y - content.min.y) * ppp).round() as i32,
                width: (r.width() * ppp).round() as i32,
                height: (r.height() * ppp).round() as i32,
                radius,
            };
            self.visible = window_shape::clip(frame, |v| {
                let area = Rect::from_min_size(content.min, vec2(v.width as f32, v.height as f32) / ppp);
                let (main, sidebar) = theme::frame_layout(area);
                vec![to_px(main.expand(theme::EDGE_MARGIN)), to_px(theme::sidebar_reach(sidebar).expand(theme::EDGE_MARGIN))]
            })
            .map(|v| vec2(v.width as f32, v.height as f32) / ppp);
            self.region_size = Some(size);
        }
        Rect::from_min_size(content.min, self.visible.unwrap_or(content.size()))
    }

    /// Launches profile `id`, remembering when it was played (orders Home).
    fn start(&mut self, ctx: &Context, id: &str) {
        let Some(root) = self.root.clone() else { return };
        let game_dir = Profiles::dir(&root, id);
        let memory = self.profiles.profiles.iter().find(|p| p.id == id).and_then(|p| p.info.max_memory_mb);
        let played = self.profiles.update(&root, id, |info| info.last_played = now_millis());
        self.report(played);
        let remembered = self.profiles.select(&root, id);
        self.report(remembered);
        self.active = Some(id.to_string());
        *self.phase.lock().unwrap() = Phase::Working(None);
        let phase = self.phase.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let report = |step: Step| {
                if !matches!(step, Step::JavaCacheSkipped(_)) {
                    *phase.lock().unwrap() = Phase::Working(Some(step));
                    ctx.request_repaint();
                }
            };
            let next = match launch::launch(&game_dir, memory, &report) {
                Ok(launched) => Phase::Running { child: launched.child, stopping: false },
                Err(e) => Phase::Failed(format!("{e:#}")),
            };
            *phase.lock().unwrap() = next;
            ctx.request_repaint();
        });
    }

    /// Returns to idle when the game exits; a crash is shown as a failure.
    fn poll_game(&self, ctx: &Context) {
        let mut phase = self.phase.lock().unwrap();
        if let Phase::Running { child, stopping } = &mut *phase {
            match child.try_wait() {
                Ok(None) => ctx.request_repaint_after(Duration::from_secs(1)),
                // Exiting after Stop is expected, whatever the exit code.
                Ok(Some(_)) if *stopping => *phase = Phase::Idle,
                Ok(Some(status)) if status.success() => *phase = Phase::Idle,
                Ok(Some(status)) => *phase = Phase::Failed(format!("Minecraft exited with {status}")),
                Err(e) => *phase = Phase::Failed(e.to_string()),
            }
        }
    }

    /// Stop: asks the game's window to close (it saves and exits like its own
    /// close button). If it has no window yet, or Stop is pressed again, the
    /// process is ended.
    fn stop_game(&self) {
        let mut phase = self.phase.lock().unwrap();
        if let Phase::Running { child, stopping } = &mut *phase {
            if *stopping || !game_window::request_close(child.id()) {
                let _ = child.kill();
            }
            *stopping = true;
        }
    }

    /// Full-height sidebar inside the window. While hovered it opens over the
    /// window and slightly past its top, bottom and left edges, the logo grows a
    /// little, and the "Hermes" wordmark appears beside it. Other contents aren't
    /// designed yet.
    ///
    /// Motion: everything moves on the app's one spring (same physics as buttons
    /// and dropdowns), layered so it reads as soft material rather than a box:
    /// the width leads; the growth past the edges and the logo chase it on their
    /// own springs, so they trail a beat; the leading (right) corners round out
    /// with speed; the wordmark waits until the panel is past halfway, then
    /// slides in, and on closing it leaves first.
    fn sidebar(&mut self, ctx: &Context, collapsed: Rect) {
        let id = Id::unique("sidebar");
        // `open` is the direction the sidebar is moving in. It stays open while the
        // pointer is anywhere over the expanded panel, and it only changes direction
        // once the width has settled: opening always fully opens, closing fully closes.
        let open = ctx.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
        let hover_area = if open { theme::sidebar_rect(collapsed, 1.0, 1.0) } else { collapsed };
        // (The debug `--open-sidebar` option opens it from frame 10, once frame timing is steady.)
        let forced = self.force_sidebar && self.frames >= 10;
        // A fast move heading out of the open sidebar closes it straight away,
        // before the pointer has actually left, so it doesn't lag behind the hand.
        let velocity = ctx.input(|i| i.pointer.velocity());
        // Coming in from the left (moving right) it waits until the pointer
        // slows down, so a fast entry from outside the window doesn't open it;
        // coming from the right (the content) opens it at any speed (owner request).
        // Right after the pointer enters the window egui has no speed for it yet
        // (it reads 0), so an entry from the left or from outside first waits
        // `theme::SIDEBAR_ENTRY_DWELL` for a real reading.
        let now = ctx.input(|i| i.time);
        let hover = ctx.pointer_hover_pos();
        let inside = hover.is_some_and(|p| collapsed.contains(p));
        let previous = ctx.data(|d| d.get_temp::<Option<egui::Pos2>>(id.with("previous"))).flatten();
        let entered_at = ctx.data(|d| d.get_temp::<f64>(id.with("entered"))).unwrap_or(f64::NEG_INFINITY);
        let from_right = previous.is_some_and(|p| p.x > collapsed.max.x);
        let entered_at = if inside && !previous.is_some_and(|p| collapsed.contains(p)) && !from_right { now } else { entered_at };
        ctx.data_mut(|d| {
            d.insert_temp(id.with("previous"), hover);
            d.insert_temp(id.with("entered"), entered_at);
        });
        let settling = !open && inside && now - entered_at < theme::SIDEBAR_ENTRY_DWELL;
        let slow = open || (!settling && (velocity.x <= 0.0 || velocity.length() < theme::EXIT_SPEED));
        if !slow {
            ctx.request_repaint_after(Duration::from_millis(15));
        }
        let wants_open = self.popup.is_none()
            && slow
            && (forced
                || ctx
                .pointer_hover_pos()
                    .is_some_and(|p| hover_area.contains(p) && !(open && theme::heading_out(hover_area, p, velocity))));
        // Leaving the window closes it at once, even mid-opening (the one exception
        // to finishing a move before reversing).
        let left_window = !forced && ctx.input(|i| i.pointer.hover_pos().is_none());
        let open = if left_window {
            false
        } else if theme::spring_peek(ctx, id).settled {
            wants_open
        } else {
            open
        };
        ctx.data_mut(|d| d.insert_temp(id, open));

        // The panel bounces a little (calmer than buttons); the logo and all text
        // settle without overshoot, so nothing inside the sidebar wobbles.
        let width = theme::spring_motion_with(ctx, id, if open { 1.0 } else { 0.0 }, theme::Bounce::Calm);
        let outset = theme::spring_motion_with(ctx, id.with("outset"), width.value, theme::Bounce::Calm);
        let grow = theme::spring_motion_with(ctx, id.with("logo"), width.value.clamp(0.0, 1.0), theme::Bounce::None);
        let reveal_target = if open && width.value > 0.55 { 1.0 } else { 0.0 };
        let word = theme::spring_motion_with(ctx, id.with("word"), reveal_target, theme::Bounce::None);

        // Opening overshoots slightly; closing never goes smaller than collapsed,
        // which would uncover the window edge it hides.
        let t = width.value.max(0.0);
        let shown = t.min(1.0);
        let rect = theme::sidebar_rect(collapsed, t, outset.value.max(0.0));
        // The leading edge softens while moving: its corners round out with speed.
        let soften = (width.velocity.abs() * 4.0).min(26.0) as u8;
        let corners = egui::CornerRadius {
            ne: theme::WINDOW_RADIUS + soften,
            se: theme::WINDOW_RADIUS + soften,
            ..theme::radius(theme::WINDOW_RADIUS)
        };
        // The logo keeps its place at the top and grows a little, a beat behind the panel.
        let pad = (theme::SIDEBAR_WIDTH - theme::LOGO_SIZE) / 2.0;
        let size = egui::emath::lerp(theme::LOGO_SIZE..=theme::LOGO_SIZE_EXPANDED, grow.value.max(0.0));
        let logo = Rect::from_min_size(rect.min + egui::Vec2::splat(pad), egui::Vec2::splat(size));
        let mut picked = None;
        egui::Area::new(id).order(egui::Order::Middle).fixed_pos(rect.min).show(ctx, |ui| {
            ui.allocate_rect(rect, Sense::hover());
            let painter = ui.painter();
            // Selected while open: the outline thickens and shimmers like a dropdown's.
            theme::glass(&painter.with_clip_rect(rect.expand(24.0)), rect, corners, shown, 0, shown, theme::Edge::Diamond);
            painter.image(self.logo.id(), logo, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);
            let reveal = word.value.clamp(0.0, 1.0);
            if reveal > 0.0 {
                // "Hermes" in Pacifico, its letters as tall as the logo: slides in from
                // just behind the logo while fading in, clipped to the panel.
                let probe = painter.layout_no_wrap("Hermes".into(), theme::wordmark_font(100.0), theme::TEXT);
                let font_size = 100.0 * size / probe.mesh_bounds.height().max(1.0);
                let color = theme::TEXT.gamma_multiply(reveal);
                let galley = painter.layout_no_wrap("Hermes".into(), theme::wordmark_font(font_size), color);
                let ink = galley.mesh_bounds;
                let slide = 10.0 * (1.0 - word.value);
                let pos = pos2(logo.max.x + 12.0 - slide - ink.min.x, logo.center().y - ink.center().y);
                painter.with_clip_rect(rect.shrink(theme::STROKE_WIDTH)).galley(pos, galley, color);
            }

            // Navigation: Home under the logo, Settings at the very bottom. Each
            // opens its page (Settings is an empty pane for now).
            let pad_top = (theme::SIDEBAR_WIDTH - theme::LOGO_SIZE) / 2.0;
            let home_y = rect.min.y + pad_top + theme::LOGO_SIZE + 18.0;
            let profiles_y = home_y + theme::NAV_ROW_HEIGHT + theme::NAV_ROW_GAP;
            let settings_y = rect.max.y - theme::NAV_INSET - theme::NAV_ROW_HEIGHT;
            let icons = &self.nav_icons;
            let rows = [
                (Page::Home, "nav_home", home_y, "sidebar.home", [&icons.home, &icons.home_filled]),
                (Page::Profiles, "nav_profiles", profiles_y, "sidebar.profiles", [&icons.profiles, &icons.profiles_filled]),
                (Page::Settings, "nav_settings", settings_y, "sidebar.settings", [&icons.settings, &icons.settings_filled]),
            ];
            for (page, id, y, label_key, icons) in rows {
                let row = NavRow { id: Id::unique(id), y, label_key, icons, selected: self.page.nav() == page };
                if nav_row(ui, rect, open, word.value, row) {
                    picked = Some(page);
                }
            }
        });
        if let Some(page) = picked {
            self.page = page;
        }
    }

    fn title_bar(&mut self, ui: &mut egui::Ui, bar: Rect) {
        let ctx = ui.ctx().clone();
        let size = vec2(40.0, 32.0);
        let close = Rect::from_center_size(pos2(bar.max.x - 32.0, bar.center().y + 1.0), size);
        let minimise = close.translate(vec2(-(size.x + 6.0), 0.0));
        // Account switcher: in the bar, left of the minimise button.
        let account = Rect::from_min_size(
            pos2(minimise.min.x - 12.0 - theme::ACCOUNT_SIZE.x, bar.center().y + 1.0 - theme::ACCOUNT_SIZE.y / 2.0),
            theme::ACCOUNT_SIZE,
        );
        // The window drags from the bar but never from its controls: the drag zone
        // stops just left of the account switcher, so pressing a control can't start a move.
        let drag_zone = Rect::from_min_max(bar.min, pos2(account.min.x - 6.0, bar.max.y));
        let drag = ui.interact(drag_zone, Id::unique("title_bar"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        // Faint divider along the bottom of the bar the window buttons sit in,
        // stopping short of the window's outline on the right.
        let y = bar.max.y - 0.5;
        ui.painter().line_segment(
            [pos2(bar.min.x, y), pos2(bar.max.x - theme::STROKE_WIDTH, y)],
            Stroke::new(1.0, theme::OUTLINE),
        );

        if bar_button(ui, close, "window.close", true, close_icon) {
            ctx.send_viewport_cmd(ViewportCommand::Close);
        }
        if bar_button(ui, minimise, "window.minimise", false, |p, c, color| {
            p.line_segment([c + vec2(-6.0, 0.0), c + vec2(6.0, 0.0)], Stroke::new(theme::ICON_STROKE, color));
        }) {
            ctx.send_viewport_cmd(ViewportCommand::Minimized(true));
        }

        let items: Vec<dropdown::Item> = self
            .accounts
            .iter()
            .map(|name| dropdown::Item { label: name.clone(), icon: Some(theme::HEAD_PLACEHOLDER) })
            .collect();
        if let Some(i) = dropdown::show(&ctx, Id::unique(ACCOUNT_DROPDOWN), account, &items, self.selected_account) {
            self.selected_account = i;
        }
    }

    /// Developer screenshot: waits for animations to settle, saves the frame, closes.
    fn screenshot_tick(&mut self, ctx: &Context) {
        let Some(path) = &self.screenshot else { return };
        self.frames += 1;
        ctx.request_repaint();
        if self.frames == 5 {
            if let Some(id) = self.focus.take() {
                ctx.memory_mut(|m| m.request_focus(id));
            }
        }
        if self.frames == self.screenshot_frame {
            let path = path.clone();
            let ctx2 = ctx.clone();
            ctx.request_screenshot(move |image| {
                let rgba: Vec<u8> = image.pixels.iter().flat_map(|p| p.to_srgba_unmultiplied()).collect();
                let [w, h] = image.size;
                if let Err(e) = image::save_buffer(&path, &rgba, w as u32, h as u32, image::ColorType::Rgba8) {
                    eprintln!("screenshot failed: {e}");
                }
                ctx2.send_viewport_cmd(ViewportCommand::Close);
            });
        }
    }
}

/// The sidebar's navigation icons: outline, and filled while that page is open.
struct NavIcons {
    home: TextureHandle,
    home_filled: TextureHandle,
    profiles: TextureHandle,
    profiles_filled: TextureHandle,
    settings: TextureHandle,
    settings_filled: TextureHandle,
}

/// One sidebar navigation row.
struct NavRow<'a> {
    id: Id,
    /// Top of the row inside the sidebar.
    y: f32,
    label_key: &'a str,
    /// Outline and filled versions.
    icons: [&'a TextureHandle; 2],
    /// Its page is the one shown.
    selected: bool,
}

/// Draws a navigation row inside the sidebar `panel`: the icon (filled while its
/// page is shown, else the outline), its label beside it (shown as the sidebar opens, on the same
/// reveal as the wordmark, `reveal` 0..1+), and, only while hovered and the
/// sidebar is `open` (not closed or closing), a frosted-glass selector of fixed
/// length (the full width of the open sidebar) around both. Returns true when clicked.
fn nav_row(ui: &mut egui::Ui, panel: Rect, open: bool, reveal: f32, nav: NavRow) -> bool {
    let NavRow { id, y, label_key, icons: [icon, filled], selected } = nav;
    let row = Rect::from_min_size(pos2(panel.min.x + theme::NAV_INSET, y), vec2(theme::NAV_ROW_WIDTH, theme::NAV_ROW_HEIGHT));
    let clip = panel.shrink(theme::STROKE_WIDTH);
    let label = tr(label_key);
    // Only the part of the row inside the sidebar responds to the pointer.
    let visible = row.intersect(clip);
    let response = ui.interact(visible, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Tab, true, selected, &label));
    let hovered = response.hovered();
    let painter = ui.painter().with_clip_rect(clip);
    // Pressing shrinks the icon, label and selector together, like every other
    // button, around the middle of the part that's visible.
    let scale = theme::press_scale(ui.ctx(), id, response.is_pointer_button_down_on());
    let origin = visible.center();
    let shrink = |r: Rect| theme::scale_about(r, origin, scale);

    let selector = ui.ctx().animate_bool_with_time(id.with("selector"), hovered && open, 0.12);
    theme::selector(&painter, shrink(row), theme::SMALL_RADIUS + 3, selector);

    // The icon sits centred in the collapsed sidebar, so it never moves sideways.
    let center = pos2(panel.min.x + theme::SIDEBAR_WIDTH / 2.0, row.center().y);
    let icon_rect = Rect::from_center_size(center, egui::Vec2::splat(theme::NAV_ICON_SIZE));
    let texture = if selected { filled } else { icon };
    painter.image(texture.id(), shrink(icon_rect), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), egui::Color32::WHITE);

    let shown = reveal.clamp(0.0, 1.0);
    if shown > 0.0 {
        let color = theme::TEXT.gamma_multiply(shown);
        let slide = 10.0 * (1.0 - reveal);
        let anchor = pos2(icon_rect.max.x + 14.0 - slide, row.center().y);
        let anchor = origin + (anchor - origin) * scale;
        painter.text(anchor, Align2::LEFT_CENTER, label, theme::text_font(theme::NAV_TEXT_SIZE * scale), color);
    }
    if response.has_focus() {
        theme::focus_ring(&painter, row.intersect(clip), theme::SMALL_RADIUS + 3);
    }
    response.clicked()
}

/// The "x" of close buttons.
pub(crate) fn close_icon(p: &egui::Painter, c: egui::Pos2, color: egui::Color32) {
    let d = 5.5;
    p.line_segment([c + vec2(-d, -d), c + vec2(d, d)], Stroke::new(theme::ICON_STROKE, color));
    p.line_segment([c + vec2(-d, d), c + vec2(d, -d)], Stroke::new(theme::ICON_STROKE, color));
}

/// A filled button with a centred label: `fill` and `ink`, lighter on hover,
/// darker when pressed, `r` corner radius.
pub(crate) fn solid_button(ui: &mut egui::Ui, rect: Rect, id: Id, label: &str, fill: egui::Color32, ink: egui::Color32, r: u8) -> bool {
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let pressed = response.is_pointer_button_down_on();
    let rect = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), id, pressed));
    let fill = if pressed {
        fill.lerp_to_gamma(theme::BG, 0.15)
    } else if response.hovered() {
        fill.lerp_to_gamma(egui::Color32::WHITE, 0.12)
    } else {
        fill
    };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(r), fill);
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), theme::text_font(15.0), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 2.0 * r as f32);
    let galley = painter.layout_job(job);
    painter.galley(Align2::CENTER_CENTER.anchor_size(rect.center(), galley.size()).min, galley, ink);
    if response.has_focus() {
        theme::focus_ring(painter, rect, r);
    }
    response.clicked()
}

/// The Profiles page's Create button: filled with the accent (owner request: Launch colour) with a
/// "+" and its label, like Add Mods.
fn create_button(ui: &mut egui::Ui, rect: Rect) -> bool {
    let plus = |p: &egui::Painter, c: egui::Pos2, color: egui::Color32| profile_popup::plus_icon(p, c, 6.0, color);
    filled_pill(ui, rect, Id::unique("profiles_create"), &tr("profiles.create"), plus, theme::PLAY, theme::PLAY_TEXT, true)
}

/// A glass pill like the account switcher, with a small icon (drawn centred on
/// the point it's given, about 12 px wide) and its label.
pub(crate) fn pill_button(ui: &mut egui::Ui, rect: Rect, id: Id, label: &str, icon: impl FnOnce(&egui::Painter, egui::Pos2, egui::Color32)) -> bool {
    let label = label.to_string();
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &label));
    let pressed = response.is_pointer_button_down_on();
    let rect = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), id, pressed));
    let corners = theme::radius(theme::CONTROL_RADIUS);
    let highlight = theme::selected_amount(ui.ctx(), id, response.hovered() || response.has_focus());
    let painter = ui.painter();
    theme::glass(&painter.with_clip_rect(rect.expand(24.0)), rect, corners, 0.0, 40, highlight, theme::Edge::Solid);
    if response.hovered() {
        painter.rect_filled(rect, corners, if pressed { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
    }
    let galley = painter.layout_no_wrap(label, theme::text_font(15.0), theme::TEXT);
    let half = 6.0;
    let gap = 8.0;
    let left = rect.center().x - (2.0 * half + gap + galley.size().x) / 2.0;
    icon(painter, pos2(left + half, rect.center().y), theme::TEXT);
    painter.galley(pos2(left + 2.0 * half + gap, rect.center().y - galley.size().y / 2.0), galley, theme::TEXT);
    if response.has_focus() {
        theme::focus_ring(painter, rect, theme::CONTROL_RADIUS);
    }
    response.clicked()
}

/// A filled pill (Launch, Add Mods): `fill` with `ink` icon and label, lighter
/// on hover, darker when pressed. While not `enabled` it's faded and can't be
/// clicked (another game is starting or running).
pub(crate) fn filled_pill(
    ui: &mut egui::Ui,
    rect: Rect,
    id: Id,
    label: &str,
    icon: impl FnOnce(&egui::Painter, egui::Pos2, egui::Color32),
    fill: egui::Color32,
    ink: egui::Color32,
    enabled: bool,
) -> bool {
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let mut response = ui.interact(rect, id, sense);
    if enabled {
        response = response.on_hover_cursor(CursorIcon::PointingHand);
    }
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let pressed = response.is_pointer_button_down_on();
    let rect = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), id, pressed));
    let fill = if !enabled {
        fill.gamma_multiply(0.45)
    } else if pressed {
        fill.lerp_to_gamma(theme::BG, 0.15)
    } else if response.hovered() {
        fill.lerp_to_gamma(egui::Color32::WHITE, 0.12)
    } else {
        fill
    };
    let ink = if enabled { ink } else { ink.gamma_multiply(0.6) };
    let painter = ui.painter();
    painter.rect_filled(rect, theme::radius(theme::CONTROL_RADIUS), fill);
    let half = 6.0;
    let gap = 8.0;
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), theme::text_font(15.0), ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(rect.width() - 2.0 * half - gap - 24.0);
    let galley = painter.layout_job(job);
    let left = rect.center().x - (2.0 * half + gap + galley.size().x) / 2.0;
    icon(painter, pos2(left + half, rect.center().y), ink);
    painter.galley(pos2(left + 2.0 * half + gap, rect.center().y - galley.size().y / 2.0), galley, ink);
    if response.has_focus() {
        theme::focus_ring(painter, rect, theme::CONTROL_RADIUS);
    }
    enabled && response.clicked()
}

/// A title-bar control. `danger` turns the hover background red (close).
/// `name_key` is the language key used as its accessible name.
pub(crate) fn bar_button(
    ui: &mut egui::Ui,
    rect: Rect,
    name_key: &str,
    danger: bool,
    icon: impl FnOnce(&egui::Painter, egui::Pos2, egui::Color32),
) -> bool {
    let response = ui.interact(rect, Id::unique(("bar_button", name_key)), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr(name_key)));
    let pressed = response.is_pointer_button_down_on();
    let rect = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), response.id, pressed));
    let active = pressed || response.hovered();
    if active {
        let fill = match (danger, pressed) {
            (true, false) => theme::DANGER,
            (true, true) => theme::DANGER.lerp_to_gamma(theme::BG, 0.2),
            (false, false) => theme::GLASS_HOVER,
            (false, true) => theme::GLASS_PRESS,
        };
        ui.painter().rect_filled(rect, theme::radius(theme::SMALL_RADIUS), fill);
    }
    if response.has_focus() {
        theme::focus_ring(ui.painter(), rect, theme::SMALL_RADIUS);
    }
    let color = if active && danger { egui::Color32::WHITE } else { theme::TEXT };
    icon(ui.painter(), rect.center(), color);
    response.clicked()
}

/// Loads each profile's icon (a PNG named in its `profile.json`); unreadable icons are skipped.
fn load_profile_icons(ctx: &Context, root: &std::path::Path, profiles: &Profiles) -> HashMap<String, TextureHandle> {
    profiles
        .profiles
        .iter()
        .filter_map(|p| {
            let bytes = std::fs::read(Profiles::icon_path(root, p)?).ok()?;
            image::load_from_memory_with_format(&bytes, image::ImageFormat::Png).ok()?;
            Some((p.id.clone(), load_png(ctx, &format!("profile-icon-{}", p.id), &bytes)))
        })
        .collect()
}

fn now_millis() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

fn load_png(ctx: &Context, name: &str, bytes: &[u8]) -> TextureHandle {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .expect("bundled PNG decodes")
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    let color = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
    // Mipmaps keep large source images smooth when drawn small (logo, profile icons).
    let options = TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..TextureOptions::LINEAR };
    ctx.load_texture(name, color, options)
}

impl eframe::App for HermesApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        // Transparent: the anti-aliased rounded edges blend with what is behind.
        [0.0; 4]
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let area = self.clip_window(&ctx, frame);
        self.poll_game(&ctx);
        self.preload_pages(&ctx);
        let (window, sidebar) = theme::frame_layout(area);

        let painter = ui.painter().clone();
        painter.rect_filled(window, theme::radius(theme::WINDOW_RADIUS), theme::BG);
        // Below the top bar, soft blobs drift slowly behind everything. 30 fps is
        // plenty for motion this slow; with reduced motion they hold still.
        let lower = Rect::from_min_max(pos2(window.min.x, window.min.y + theme::BAR_HEIGHT), window.max);
        theme::backdrop(&painter, lower, ctx.input(|i| i.time));
        if !theme::reduce_motion() {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        theme::outline(&painter, window, theme::WINDOW_RADIUS, theme::Edge::Diamond);

        // Main content pane, in liquid glass, down to the bottom of the window:
        // Home lists the pinned and recently played profiles in the right half,
        // Profiles lists every profile across the full width under its Create
        // button, Settings isn't designed yet (empty, right half).
        // A profile's page uses the full width for its own panes.
        let body = theme::body_layout(window, sidebar);
        match self.page {
            Page::Profiles => {
                let button = Rect::from_min_size(pos2(body.wide.max.x - theme::CREATE_BUTTON_SIZE.x, body.wide.min.y), theme::CREATE_BUTTON_SIZE);
                if create_button(ui, button) {
                    self.popup = Some(ProfilePopup::create());
                }
                let pane = Rect::from_min_max(pos2(body.wide.min.x, button.max.y + theme::GAP), body.wide.max);
                theme::pane(&painter, pane, theme::PANEL_RADIUS);
                self.profile_cards(ui, pane, false);
            }
            Page::Home => {
                theme::pane(&painter, body.content, theme::PANEL_RADIUS);
                self.profile_cards(ui, body.content, true);
            }
            Page::Settings => theme::pane(&painter, body.content, theme::PANEL_RADIUS),
            Page::Profile => self.profile_page_ui(ui, body.wide),
        }

        self.sidebar(&ctx, sidebar);

        // Custom bar across the top of the window, right of the sidebar.
        let bar = Rect::from_min_max(pos2(sidebar.max.x, window.min.y), pos2(window.max.x, window.min.y + theme::BAR_HEIGHT));
        self.title_bar(ui, bar);

        self.profile_popup(&ctx, window);

        self.screenshot_tick(&ctx);
    }
}

/// Window and taskbar icon: the logo on a dark blue circle (the exe file gets the
/// same artwork from `app_icon.ico`, see build.rs).
fn app_icon() -> egui::IconData {
    let image = image::load_from_memory_with_format(include_bytes!("../assets/images/app_icon.png"), image::ImageFormat::Png)
        .expect("bundled PNG decodes")
        .to_rgba8();
    egui::IconData { width: image.width(), height: image.height(), rgba: image.into_raw() }
}

pub fn run(dev: DevOptions) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_icon(app_icon())
            .with_inner_size(theme::window_size())
            .with_decorations(false)
            .with_transparent(true)
            .with_resizable(false)
            .with_maximize_button(false)
            // Images can be dropped on the create-profile popup.
            .with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        "Hermes Client",
        options,
        Box::new(|cc| {
            window_shape::keep_caption_hidden(cc);
            Ok(Box::new(HermesApp::new(&cc.egui_ctx, dev)))
        }),
    )
}
