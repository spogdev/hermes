//! A profile's own page, opened by clicking its card. Left: the profile (icon,
//! name, client and Minecraft version, and buttons to open its folder, share it
//! and edit it) over a pane describing one mod (the hovered one, else the last
//! hovered, else the first). Right: a table of the profile's mods (turn each on
//! or off, or remove it), or, while browsing, Modrinth search results to install,
//! a page at a time. Fabric API is always there and can't be turned off.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{self, pos2, vec2, Align2, Color32, Context, CursorIcon, Id, Painter, Pos2, Rect, Sense, Stroke, TextureHandle};

use crate::lang::{tr, tr_args};
use crate::mods::{self, InstalledMod, Project};
use crate::profiles::{Profile, Profiles};
use crate::{profile_list, shell, theme};

/// Typing pauses this long (seconds) before the search runs.
const SEARCH_DELAY: f64 = 0.35;
/// A first click on remove arms it for this long (seconds); a second click removes.
const DELETE_ARM_TIME: f64 = 3.0;

/// The header buttons' icons (owner artwork): outline, and filled while hovered.
pub struct Icons {
    pub folder: [TextureHandle; 2],
    pub share: [TextureHandle; 2],
    pub settings: [TextureHandle; 2],
}

/// What the page asks the app to do.
pub enum Action {
    None,
    /// Open the profile popup to edit this profile.
    Settings,
    /// Launch this profile.
    Launch,
    /// Close its running game.
    Stop,
}

/// Written by background work, read by the page each frame.
#[derive(Default)]
struct Shared {
    /// Bumped whenever `mods`, `projects` or `results` change.
    revision: u64,
    scanning: bool,
    mods: Vec<InstalledMod>,
    /// Modrinth projects of the installed jars, by jar sha1.
    projects: HashMap<String, Project>,
    searching: bool,
    /// Only the newest search may write its results.
    search_generation: u64,
    results: Vec<Project>,
    /// Matches across all result pages.
    total: usize,
    /// Project ids being installed.
    installing: HashSet<String>,
    /// Icons decoded in the background, waiting to become textures (`None`: unreadable).
    icons: Vec<(String, Option<image::RgbaImage>)>,
    error: Option<String>,
}

/// Which mod the detail pane describes.
#[derive(Clone, PartialEq)]
enum Key {
    /// An installed jar, by sha1 (stays the same when it's turned off).
    Installed(String),
    /// A search result, by Modrinth project id.
    Result(String),
}

pub struct ProfilePage {
    dir: PathBuf,
    minecraft: String,
    ctx: Context,
    shared: Arc<Mutex<Shared>>,
    // A copy of `shared`, refreshed when its revision changes.
    seen: u64,
    mods: Vec<InstalledMod>,
    projects: HashMap<String, Project>,
    results: Vec<Project>,
    total: usize,
    /// The result page shown (from 0).
    page: usize,
    installing: HashSet<String>,
    scanning: bool,
    searching: bool,
    error: Option<String>,
    /// Mod icons: `jar:<sha1>` for a jar's own icon, else the icon URL.
    textures: HashMap<String, TextureHandle>,
    requested_icons: HashSet<String>,
    last: Option<Key>,
    browsing: bool,
    query: String,
    /// When the search text last changed (UI clock), until that search runs.
    query_edited: Option<f64>,
    /// The jar whose remove button was clicked once, and when.
    delete_armed: Option<(String, f64)>,
}

impl ProfilePage {
    pub fn open(ctx: &Context, root: &Path, profile: &Profile) -> Self {
        let page = ProfilePage {
            dir: Profiles::dir(root, &profile.id),
            minecraft: profile.info.minecraft.clone().unwrap_or_else(|| crate::launch::MC_VERSION.to_string()),
            ctx: ctx.clone(),
            shared: Arc::default(),
            seen: 0,
            mods: Vec::new(),
            projects: HashMap::new(),
            results: Vec::new(),
            total: 0,
            page: 0,
            installing: HashSet::new(),
            scanning: true,
            searching: false,
            error: None,
            textures: HashMap::new(),
            requested_icons: HashSet::new(),
            last: None,
            browsing: false,
            query: String::new(),
            query_edited: None,
            delete_armed: None,
        };
        rescan(&page.shared, &page.dir, &page.ctx);
        // Every profile has Fabric API (owner request): added (or turned back on) if it isn't.
        let (shared, dir, ctx, mc) = (page.shared.clone(), page.dir.clone(), ctx.clone(), page.minecraft.clone());
        mods::runtime().spawn(async move {
            let ensured = match mods::target(&mc).await {
                Ok(target) => mods::ensure_fabric_api(&dir, &target).await,
                Err(e) => Err(e),
            };
            match ensured {
                Ok(true) => rescan(&shared, &dir, &ctx),
                Ok(false) => {}
                Err(e) => {
                    shared.lock().unwrap().error = Some(format!("{e:#}"));
                    ctx.request_repaint();
                }
            }
        });
        // Browsing opens on results that are already there (owner request).
        let mut page = page;
        page.search();
        page
    }

    /// Whether it's still reading its mods or fetching search results.
    pub fn loading(&self) -> bool {
        let s = self.shared.lock().unwrap();
        s.scanning || s.searching
    }

    /// Opened again: back to the installed mods, re-read in the background
    /// (what's shown stays until the new list is in).
    pub fn reopen(&mut self) {
        self.browsing = false;
        rescan(&self.shared, &self.dir, &self.ctx);
    }

    /// Switches the table to Modrinth search results (the most downloaded first).
    pub fn browse(&mut self) {
        self.browsing = true;
        if self.results.is_empty() && !self.searching {
            self.search();
        }
    }

    /// Runs the search for the text typed, from its first page.
    fn search(&mut self) {
        self.page = 0;
        self.fetch_page();
    }

    fn fetch_page(&mut self) {
        self.query_edited = None;
        let page = self.page;
        let (shared, ctx, query, mc) = (self.shared.clone(), self.ctx.clone(), self.query.clone(), self.minecraft.clone());
        let generation = {
            let mut s = shared.lock().unwrap();
            s.search_generation += 1;
            s.searching = true;
            s.error = None;
            s.search_generation
        };
        mods::runtime().spawn(async move {
            let found = mods::search(&query, &mc, page).await;
            let mut s = shared.lock().unwrap();
            if s.search_generation != generation {
                return;
            }
            s.searching = false;
            match found {
                Ok(found) => {
                    s.results = found.projects;
                    s.total = found.total;
                }
                Err(e) => s.error = Some(format!("{e:#}")),
            }
            s.revision += 1;
            ctx.request_repaint();
        });
    }

    fn install(&self, project: String) {
        let installed: HashSet<String> = self.projects.values().map(|p| p.id.clone()).collect();
        let (shared, ctx, dir, mc) = (self.shared.clone(), self.ctx.clone(), self.dir.clone(), self.minecraft.clone());
        shared.lock().unwrap().installing.insert(project.clone());
        mods::runtime().spawn(async move {
            let result = match mods::target(&mc).await {
                Ok(target) => mods::install(&dir, &project, &target, &installed).await,
                Err(e) => Err(e),
            };
            {
                let mut s = shared.lock().unwrap();
                s.installing.remove(&project);
                if let Err(e) = result {
                    s.error = Some(format!("{e:#}"));
                }
            }
            rescan(&shared, &dir, &ctx);
        });
    }

    fn request_icon(&mut self, url: &str) {
        if !self.requested_icons.insert(url.to_string()) {
            return;
        }
        let (shared, ctx, url) = (self.shared.clone(), self.ctx.clone(), url.to_string());
        mods::runtime().spawn(async move {
            let image = mods::icon(url.clone()).await;
            shared.lock().unwrap().icons.push((url, image));
            ctx.request_repaint();
        });
    }

    /// Copies what background work has written, and turns new icons into textures.
    fn sync(&mut self) {
        let mut s = self.shared.lock().unwrap();
        if s.revision != self.seen {
            self.seen = s.revision;
            self.mods = s.mods.iter().map(InstalledMod::clone_without_icon).collect();
            self.projects = s.projects.clone();
            self.results = s.results.clone();
            self.total = s.total;
        }
        self.installing = s.installing.clone();
        self.scanning = s.scanning;
        self.searching = s.searching;
        self.error = s.error.clone();
        for (key, image) in s.icons.drain(..) {
            if let Some(img) = image {
                let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
                let options = egui::TextureOptions { mipmap_mode: Some(egui::TextureFilter::Linear), ..egui::TextureOptions::LINEAR };
                self.textures.insert(key.clone(), self.ctx.load_texture(format!("mod-icon-{key}"), color, options));
            }
        }
        drop(s);
        // Result icons load as soon as results arrive, before browsing opens.
        let urls: Vec<String> = self.results.iter().filter_map(|p| p.icon_url.clone()).collect();
        for url in urls {
            self.request_icon(&url);
        }
    }

    /// The icon to show for a mod: its Modrinth icon (fetched on first use), else
    /// the jar's own.
    fn icon(&mut self, url: Option<&str>, jar_sha1: Option<&str>) -> Option<TextureHandle> {
        if let Some(url) = url {
            self.request_icon(url);
            if let Some(t) = self.textures.get(url) {
                return Some(t.clone());
            }
        }
        jar_sha1.and_then(|sha1| self.textures.get(&format!("jar:{sha1}")).cloned())
    }

    /// `status`: what this profile's game is doing (as its card shows it).
    pub fn show(&mut self, ui: &mut egui::Ui, area: Rect, profile: &Profile, icon: Option<&TextureHandle>, icons: &Icons, status: &profile_list::Status) -> Action {
        self.sync();
        let now = ui.input(|i| i.time);
        if self.delete_armed.as_ref().is_some_and(|(_, at)| now - at > DELETE_ARM_TIME) {
            self.delete_armed = None;
        }
        let left = Rect::from_min_max(area.min, pos2(area.min.x + (area.width() * theme::PROFILE_LEFT_FRACTION).round(), area.max.y));
        let right = Rect::from_min_max(pos2(left.max.x + theme::GAP, area.min.y), area.max);

        let header = Rect::from_min_size(left.min, vec2(left.width(), theme::PROFILE_ICON_SIZE));
        let action = header_ui(ui, header, profile, &self.minecraft, icon, icons, &self.dir, status);

        // Above the table: Browse (or back to the installed mods), and the search field while browsing.
        let strip = Rect::from_min_size(right.min, vec2(right.width(), theme::CREATE_BUTTON_SIZE.y));
        let pill = Rect::from_min_size(pos2(strip.max.x - theme::CREATE_BUTTON_SIZE.x, strip.min.y), theme::CREATE_BUTTON_SIZE);
        if self.browsing {
            if crate::ui::pill_button(ui, pill, Id::unique("mods_installed"), &tr("mods.installed"), back_icon) {
                self.browsing = false;
            }
            self.search_field(ui, Rect::from_min_max(strip.min, pos2(pill.min.x - theme::GAP, strip.max.y)), now);
        } else if crate::ui::filled_pill(
            ui,
            pill,
            Id::unique("mods_browse"),
            &tr("mods.browse"),
            |p, c, color| crate::profile_popup::plus_icon(p, c, 6.0, color),
            theme::PLAY,
            theme::PLAY_TEXT,
            true,
        ) {
            self.browse();
        }

        // While browsing, the page buttons sit between the search and the results.
        let mut table_top = strip.max.y + theme::GAP;
        if self.browsing {
            let pager = Rect::from_min_size(pos2(right.min.x, table_top), vec2(right.width(), theme::PAGER_HEIGHT));
            if let Some(page) = pager_ui(ui, pager, self.page, self.total.div_ceil(mods::PAGE_SIZE)) {
                self.page = page;
                self.fetch_page();
            }
            table_top = pager.max.y + theme::GAP;
        }
        let table = Rect::from_min_max(pos2(right.min.x, table_top), right.max);
        theme::pane(ui.painter(), table, theme::PANEL_RADIUS);
        let hovered = self.table(ui, table, now);

        let detail = Rect::from_min_max(pos2(left.min.x, header.max.y + theme::GAP), left.max);
        theme::pane(ui.painter(), detail, theme::PANEL_RADIUS);
        if hovered.is_some() {
            self.last = hovered;
        }
        let shown = self.shown();
        if let Some(d) = shown {
            detail_ui(ui, detail, d);
        }
        action
    }

    fn search_field(&mut self, ui: &mut egui::Ui, rect: Rect, now: f64) {
        let painter = ui.painter();
        let corners = theme::radius(theme::CONTROL_RADIUS);
        painter.rect_filled(rect, corners, theme::GLASS_TINT);
        let glass = Pos2::new(rect.min.x + 20.0, rect.center().y);
        search_icon(painter, glass, theme::TEXT.gamma_multiply(0.7));
        let edit_rect = Rect::from_min_max(pos2(glass.x + 16.0, rect.min.y), pos2(rect.max.x - 12.0, rect.max.y));
        let hint = egui::RichText::new(tr("mods.search")).font(theme::regular_font(15.0)).color(theme::TEXT.gamma_multiply(0.5));
        let edit = egui::TextEdit::singleline(&mut self.query)
            .id(Id::unique("mods_search"))
            .frame(egui::Frame::NONE)
            .font(theme::text_font(15.0))
            .text_color(theme::TEXT)
            .hint_text(hint)
            .char_limit(100)
            .align(Align2::LEFT_CENTER)
            .desired_width(edit_rect.width());
        let response = ui.put(edit_rect, edit);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::SearchInput, true, tr("mods.search")));
        let focus = theme::selected_amount(ui.ctx(), response.id, response.has_focus() || response.hovered());
        theme::selectable_outline(ui.painter(), rect, corners, focus, 0.0, theme::Edge::Solid);
        if response.changed() {
            self.query_edited = Some(now);
        }
        let enter = response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        match self.query_edited {
            _ if enter => self.search(),
            Some(at) if now - at >= SEARCH_DELAY => self.search(),
            Some(_) => ui.ctx().request_repaint_after(Duration::from_millis(50)),
            None => {}
        }
    }

    /// Draws the table and acts on its buttons; returns the hovered row's mod.
    fn table(&mut self, ui: &mut egui::Ui, pane: Rect, now: f64) -> Option<Key> {
        let rows: Vec<Row> = if self.browsing {
            let results = self.results.clone();
            let installed: HashSet<&str> = self.projects.values().map(|p| p.id.as_str()).collect();
            let states: Vec<Options> = results
                .iter()
                .map(|p| Options::Install { installed: installed.contains(p.id.as_str()), installing: self.installing.contains(&p.id) })
                .collect();
            results
                .into_iter()
                .zip(states)
                .map(|(p, options)| Row {
                    key: Key::Result(p.id.clone()),
                    icon: self.icon(p.icon_url.as_deref(), None),
                    title: p.title,
                    author: p.author.unwrap_or_default(),
                    third: tr_args("mod.downloads", &[&mods::short_count(p.downloads)]),
                    options,
                })
                .collect()
        } else {
            let list: Vec<(String, String, String, Option<String>, Option<String>, bool, String, bool)> = self
                .mods
                .iter()
                .map(|m| {
                    let project = self.projects.get(&m.sha1);
                    (
                        m.sha1.clone(),
                        project.map_or_else(|| m.name.clone(), |p| p.title.clone()),
                        project.and_then(|p| p.author.clone()).unwrap_or_else(|| m.authors.join(", ")),
                        m.version.clone(),
                        project.and_then(|p| p.icon_url.clone()),
                        m.enabled,
                        m.file.clone(),
                        m.id.as_deref() == Some(mods::FABRIC_API_ID),
                    )
                })
                .collect();
            list.into_iter()
                .map(|(sha1, title, author, version, icon_url, enabled, file, locked)| Row {
                    icon: self.icon(icon_url.as_deref(), Some(&sha1)),
                    key: Key::Installed(sha1),
                    title,
                    author,
                    third: version.unwrap_or_default(),
                    options: if locked {
                        Options::Locked
                    } else {
                        Options::Installed { enabled, armed: self.delete_armed.as_ref().is_some_and(|(f, _)| *f == file) }
                    },
                })
                .collect()
        };

        let third_header = if self.browsing { "mods.downloads" } else { "mods.version" };
        let headers = [tr("mods.project"), tr("mods.author"), tr(third_header), tr("mods.options")];
        let salt = if self.browsing { "mods_results" } else { "mods_installed" };
        let (hovered, clicked) = table_ui(ui, pane, salt, &headers, &rows);

        // Loading, empty and error states.
        let painter = ui.painter();
        let body = Rect::from_min_max(pos2(pane.min.x, pane.min.y + theme::TABLE_HEADER_HEIGHT), pane.max);
        let busy = if self.browsing { self.searching } else { self.scanning };
        if rows.is_empty() {
            if busy {
                profile_list::spinner(painter, body.center(), 13.0, None, now as f32, theme::PLAY);
                ui.ctx().request_repaint();
            } else {
                let key = if self.browsing { "mods.no_results" } else { "mods.empty" };
                painter.text(body.center(), Align2::CENTER_CENTER, tr(key), theme::regular_font(15.0), theme::TEXT.gamma_multiply(0.6));
            }
        } else if busy && self.browsing {
            // A newer search is on its way: a small ring by the headers.
            profile_list::spinner(painter, pos2(pane.max.x - 22.0, pane.min.y + theme::TABLE_HEADER_HEIGHT / 2.0), 7.0, None, now as f32, theme::PLAY);
            ui.ctx().request_repaint();
        }
        if !self.installing.is_empty() {
            ui.ctx().request_repaint();
        }
        if let Some(error) = &self.error {
            let mut job = egui::text::LayoutJob::simple(error.clone(), theme::regular_font(13.0), theme::DANGER_TEXT, pane.width() - 2.0 * theme::GAP);
            job.wrap.max_rows = 2;
            let galley = painter.layout_job(job);
            painter.galley(pos2(pane.min.x + theme::GAP, pane.max.y - theme::GAP - galley.size().y), galley, theme::DANGER_TEXT);
        }

        if let Some((i, click)) = clicked {
            match (click, &rows[i].key) {
                (Click::Toggle, Key::Installed(sha1)) => {
                    if let Some(m) = self.mods.iter_mut().find(|m| m.sha1 == *sha1) {
                        match mods::set_enabled(&self.dir, &m.file, !m.enabled) {
                            Ok(file) => {
                                m.file = file;
                                m.enabled = !m.enabled;
                            }
                            Err(e) => self.shared.lock().unwrap().error = Some(format!("{e:#}")),
                        }
                        rescan(&self.shared, &self.dir, &self.ctx);
                    }
                }
                (Click::Remove, Key::Installed(sha1)) => {
                    if let Some(m) = self.mods.iter().find(|m| m.sha1 == *sha1) {
                        let file = m.file.clone();
                        if self.delete_armed.as_ref().is_some_and(|(f, _)| *f == file) {
                            self.delete_armed = None;
                            if let Err(e) = mods::remove(&self.dir, &file) {
                                self.shared.lock().unwrap().error = Some(format!("{e:#}"));
                            }
                            self.mods.retain(|m| m.file != file);
                            rescan(&self.shared, &self.dir, &self.ctx);
                        } else {
                            self.delete_armed = Some((file, now));
                        }
                    }
                }
                (Click::Install, Key::Result(id)) => self.install(id.clone()),
                _ => {}
            }
        }
        hovered.map(|i| rows[i].key.clone())
    }

    /// The mod the detail pane describes: the last hovered one still in the
    /// table, else the table's first.
    fn shown(&mut self) -> Option<Detail> {
        let in_table = |key: &Key| match key {
            Key::Installed(sha1) => !self.browsing && self.mods.iter().any(|m| m.sha1 == *sha1),
            Key::Result(id) => self.browsing && self.results.iter().any(|p| p.id == *id),
        };
        let key = match &self.last {
            Some(key) if in_table(key) => key.clone(),
            _ if self.browsing => Key::Result(self.results.first()?.id.clone()),
            _ => Key::Installed(self.mods.first()?.sha1.clone()),
        };
        match key {
            Key::Result(id) => {
                let p = self.results.iter().find(|p| p.id == id)?.clone();
                Some(Detail {
                    icon: self.icon(p.icon_url.as_deref(), None),
                    title: p.title.clone(),
                    author: p.author.clone(),
                    downloads: Some(p.downloads),
                    description: Some(p.description.clone()).filter(|d| !d.is_empty()),
                    link: Some(p.page_url()),
                })
            }
            Key::Installed(sha1) => {
                let m = self.mods.iter().find(|m| m.sha1 == sha1)?;
                let project = self.projects.get(&sha1).cloned();
                let (name, authors, description) = (m.name.clone(), m.authors.join(", "), m.description.clone());
                Some(Detail {
                    icon: self.icon(project.as_ref().and_then(|p| p.icon_url.as_deref()), Some(&sha1)),
                    title: project.as_ref().map_or(name, |p| p.title.clone()),
                    author: project.as_ref().and_then(|p| p.author.clone()).or(Some(authors).filter(|a| !a.is_empty())),
                    downloads: project.as_ref().map(|p| p.downloads),
                    description: project.as_ref().map(|p| p.description.clone()).filter(|d| !d.is_empty()).or(description),
                    link: project.as_ref().map(Project::page_url),
                })
            }
        }
    }
}

/// Scans the profile's mods folder, then asks Modrinth what the jars are.
fn rescan(shared: &Arc<Mutex<Shared>>, dir: &Path, ctx: &Context) {
    shared.lock().unwrap().scanning = true;
    let (shared, dir, ctx) = (shared.clone(), dir.to_path_buf(), ctx.clone());
    mods::runtime().spawn(async move {
        let scanned = tokio::task::spawn_blocking(move || mods::scan(&dir)).await;
        let mut list = match scanned {
            Ok(Ok(list)) => list,
            Ok(Err(e)) => {
                let mut s = shared.lock().unwrap();
                s.scanning = false;
                s.error = Some(format!("{e:#}"));
                ctx.request_repaint();
                return;
            }
            Err(_) => return,
        };
        let sha1s: Vec<String> = list.iter().map(|m| m.sha1.clone()).collect();
        {
            let mut s = shared.lock().unwrap();
            for m in &mut list {
                if let Some(icon) = m.icon.take() {
                    s.icons.push((format!("jar:{}", m.sha1), Some(icon)));
                }
            }
            s.mods = list;
            s.scanning = false;
            s.revision += 1;
        }
        ctx.request_repaint();
        // Offline or unknown jars just keep what the jar says about itself.
        if let Ok(found) = mods::identify(sha1s).await {
            let mut s = shared.lock().unwrap();
            s.projects = found;
            s.revision += 1;
            ctx.request_repaint();
        }
    });
}

/// One table row, ready to draw.
struct Row {
    key: Key,
    icon: Option<TextureHandle>,
    title: String,
    author: String,
    /// Version (installed) or download count (search results).
    third: String,
    options: Options,
}

enum Options {
    /// A switch to turn the mod on or off, and remove (`armed`: clicked once, click again to remove).
    Installed { enabled: bool, armed: bool },
    /// Install, or a tick once it's in the profile.
    Install { installed: bool, installing: bool },
    /// Fabric API: always installed and on, so no buttons.
    Locked,
}

#[derive(Clone, Copy)]
enum Click {
    Toggle,
    Remove,
    Install,
}

/// What the detail pane shows.
struct Detail {
    icon: Option<TextureHandle>,
    title: String,
    author: Option<String>,
    downloads: Option<u64>,
    description: Option<String>,
    /// The mod's Modrinth page.
    link: Option<String>,
}

/// Column starts as fractions of the table's inner width.
fn columns(inner: Rect) -> [f32; 5] {
    let w = inner.width();
    let [a, b, c, d] = theme::TABLE_COLUMNS;
    [inner.min.x + a * w, inner.min.x + b * w, inner.min.x + c * w, inner.min.x + d * w, inner.max.x]
}

/// Draws the header and the scrolling rows; returns the hovered row and any button clicked.
fn table_ui(ui: &mut egui::Ui, pane: Rect, salt: &str, headers: &[String; 4], rows: &[Row]) -> (Option<usize>, Option<(usize, Click)>) {
    let inner = pane.shrink2(vec2(theme::TABLE_PADDING, 0.0));
    let cols = columns(inner);
    let painter = ui.painter();
    let header_y = pane.min.y + theme::TABLE_HEADER_HEIGHT / 2.0;
    let label_color = theme::TEXT.gamma_multiply(0.65);
    for (i, label) in headers.iter().enumerate().take(3) {
        truncated(painter, label, theme::regular_font(14.0), label_color, pos2(cols[i], header_y), cols[i + 1] - cols[i] - 8.0);
    }
    // Options sits over its buttons, at the right.
    painter.text(pos2(cols[4], header_y), Align2::RIGHT_CENTER, &headers[3], theme::regular_font(14.0), label_color);
    let line_y = pane.min.y + theme::TABLE_HEADER_HEIGHT;
    painter.line_segment([pos2(inner.min.x, line_y), pos2(inner.max.x, line_y)], Stroke::new(1.0, theme::OUTLINE));

    let area = Rect::from_min_max(pos2(pane.min.x + theme::STROKE_WIDTH, line_y + 1.0), pane.max - vec2(theme::STROKE_WIDTH, theme::STROKE_WIDTH));
    let mut child = ui.new_child(egui::UiBuilder::new().id_salt(salt).max_rect(area).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(area.intersect(ui.clip_rect()));
    let mut hovered = None;
    let mut clicked = None;
    egui::ScrollArea::vertical().id_salt(salt).auto_shrink([false; 2]).show(&mut child, |ui| {
        ui.add_space(6.0);
        for (i, row) in rows.iter().enumerate() {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), theme::MOD_ROW_HEIGHT), Sense::hover());
            if !ui.is_rect_visible(rect) {
                continue;
            }
            // Hovered rows get the sidebar's selection box (owner request).
            let highlight = Rect::from_min_max(pos2(inner.min.x - 8.0, rect.min.y + 2.0), pos2(inner.max.x + 8.0, rect.max.y - 2.0));
            let is_hovered = ui.rect_contains_pointer(highlight);
            if is_hovered {
                hovered = Some(i);
            }
            let shown = ui.ctx().animate_bool_with_time(Id::unique((salt, "selector", i)), is_hovered, 0.12);
            theme::selector(ui.painter(), highlight, theme::SMALL_RADIUS + 3, shown);
            if let Some(click) = row_ui(ui, rect, &cols, Id::unique((salt, i)), row) {
                clicked = Some((i, click));
            }
        }
        ui.add_space(6.0);
    });
    (hovered, clicked)
}

fn row_ui(ui: &mut egui::Ui, rect: Rect, cols: &[f32; 5], id: Id, row: &Row) -> Option<Click> {
    let painter = ui.painter().clone();
    let y = rect.center().y;
    let dim = if matches!(row.options, Options::Installed { enabled: false, .. }) { 0.45 } else { 1.0 };
    let icon = Rect::from_min_size(pos2(cols[0], y - theme::MOD_ICON_SIZE / 2.0), egui::Vec2::splat(theme::MOD_ICON_SIZE));
    mod_icon(&painter, icon, row.icon.as_ref(), theme::SMALL_RADIUS - 2, dim);
    let text_x = icon.max.x + 10.0;
    truncated(&painter, &row.title, theme::text_font(15.0), theme::TEXT.gamma_multiply(dim), pos2(text_x, y), cols[1] - text_x - 10.0);
    let detail = theme::TEXT.gamma_multiply(0.7 * dim);
    truncated(&painter, &row.author, theme::regular_font(14.0), detail, pos2(cols[1], y), cols[2] - cols[1] - 10.0);
    truncated(&painter, &row.third, theme::regular_font(14.0), detail, pos2(cols[2], y), cols[3] - cols[2] - 10.0);

    let mut click = None;
    match row.options {
        // Buttons line up on the right of the Options column (owner request).
        Options::Installed { enabled, armed } => {
            let remove = Rect::from_min_size(pos2(cols[4] - 30.0, y - 15.0), vec2(30.0, 30.0));
            if remove_button(ui, remove, id.with("remove"), armed) {
                click = Some(Click::Remove);
            }
            let switch = Rect::from_min_size(pos2(remove.min.x - 10.0 - 38.0, y - 11.0), vec2(38.0, 22.0));
            if toggle_ui(ui, switch, id.with("toggle"), enabled) {
                click = Some(Click::Toggle);
            }
        }
        Options::Locked => {}
        Options::Install { installed, installing } => {
            let button = Rect::from_min_size(pos2(cols[4] - 30.0, y - 15.0), vec2(30.0, 30.0));
            if installing {
                profile_list::spinner(&painter, button.center(), 9.0, None, ui.input(|i| i.time) as f32, theme::PLAY);
            } else if installed {
                check_icon(&painter, button.center(), theme::PLAY);
            } else if circle_button(ui, button, id.with("install"), "mod.install", |p, c, color, _| crate::profile_popup::plus_icon(p, c, 6.0, color)) {
                click = Some(Click::Install);
            }
        }
    }
    click
}

/// Draws `text` on one line from `left` (vertically centred on `left.y`), cut
/// short with an ellipsis past `width`.
fn truncated(painter: &Painter, text: &str, font: egui::FontId, color: Color32, left: Pos2, width: f32) {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_string(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(width.max(0.0));
    let galley = painter.layout_job(job);
    painter.galley(Align2::LEFT_CENTER.anchor_size(left, galley.size()).min, galley, color);
}

/// A mod's icon, or a "?" tile without one.
fn mod_icon(painter: &Painter, rect: Rect, texture: Option<&TextureHandle>, r: u8, alpha: f32) {
    let corners = theme::radius(r);
    match texture {
        Some(t) => {
            let tint = Color32::WHITE.gamma_multiply(alpha);
            painter.add(egui::epaint::RectShape::filled(rect, corners, tint).with_texture(t.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))));
        }
        None => {
            painter.rect_filled(rect, corners, theme::GLASS_HOVER);
            painter.text(rect.center(), Align2::CENTER_CENTER, "?", theme::text_font(rect.height() * 0.45), theme::TEXT.gamma_multiply(0.6 * alpha));
        }
    }
}

/// The profile: its icon, name, versions and the folder, share and settings buttons.
fn header_ui(ui: &mut egui::Ui, rect: Rect, profile: &Profile, minecraft: &str, icon: Option<&TextureHandle>, icons: &Icons, dir: &Path, status: &profile_list::Status) -> Action {
    let painter = ui.painter().clone();
    let tile = Rect::from_min_size(rect.min, egui::Vec2::splat(theme::PROFILE_ICON_SIZE));
    match icon {
        Some(t) => {
            painter.add(egui::epaint::RectShape::filled(tile, theme::radius(theme::CONTROL_RADIUS), Color32::WHITE).with_texture(t.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0))));
        }
        None => {
            painter.rect_filled(tile, theme::radius(theme::CONTROL_RADIUS), theme::GLASS_HOVER);
            painter.text(tile.center(), Align2::CENTER_CENTER, "?", theme::text_font(52.0), theme::TEXT.gamma_multiply(0.6));
        }
    }
    let x = tile.max.x + 20.0;
    let width = rect.max.x - x;
    let name = profile.info.name.clone().unwrap_or_else(|| tr("profile.unnamed"));
    truncated(&painter, &name, theme::text_font(30.0), theme::TEXT, pos2(x, rect.min.y + 22.0), width);
    truncated(&painter, &tr_args("profile.version", &[minecraft]), theme::regular_font(17.0), theme::TEXT.gamma_multiply(0.7), pos2(x, rect.min.y + 56.0), width);

    // Launch first (owner request), then the round buttons, all one height.
    let d = theme::CIRCLE_BUTTON_SIZE;
    let launch = Rect::from_min_size(pos2(x, rect.max.y - d), vec2(theme::LAUNCH_BUTTON_WIDTH, d));
    let launched = launch_button(ui, launch, status);
    let first = launch.max.x + theme::HEADER_BUTTON_GAP;
    let at = |i: f32| Rect::from_min_size(pos2(first + i * (d + theme::HEADER_BUTTON_GAP), rect.max.y - d), egui::Vec2::splat(d));
    if circle_button(ui, at(0.0), Id::unique("profile_open_folder"), "profile.open_folder", image(&icons.folder, 22.0)) {
        let _ = std::fs::create_dir_all(dir);
        shell::open(dir);
    }
    // Share: no function yet (owner request).
    circle_button(ui, at(1.0), Id::unique("profile_share"), "profile.share", image(&icons.share, 21.0));
    let settings_clicked = circle_button(ui, at(2.0), Id::unique("profile_settings"), "profile.settings", image(&icons.settings, 24.0));
    match launched {
        Some(action) => action,
        None if settings_clicked => Action::Settings,
        None => Action::None,
    }
}

/// Draws an outline icon, or its filled version while the button is hovered.
fn image(pair: &[TextureHandle; 2], size: f32) -> impl FnOnce(&Painter, Pos2, Color32, bool) + '_ {
    move |p, c, color, active| {
        let r = Rect::from_center_size(c, egui::Vec2::splat(size));
        p.image(pair[active as usize].id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), color);
    }
}

/// The mod described in the left pane: icon, name, author and downloads, a link
/// to its Modrinth page, and its description in a box below.
fn detail_ui(ui: &mut egui::Ui, pane: Rect, d: Detail) {
    let painter = ui.painter().with_clip_rect(pane.shrink(theme::STROKE_WIDTH));
    let inner = pane.shrink(theme::DETAIL_PADDING);
    let icon = Rect::from_min_size(inner.min, egui::Vec2::splat(theme::MOD_DETAIL_ICON_SIZE));
    mod_icon(&painter, icon, d.icon.as_ref(), theme::CONTROL_RADIUS, 1.0);
    let x = icon.max.x + 16.0;
    let width = inner.max.x - x;
    truncated(&painter, &d.title, theme::text_font(21.0), theme::TEXT, pos2(x, icon.min.y + 13.0), width);

    // Author, then the download count after a gap.
    let detail = theme::TEXT.gamma_multiply(0.7);
    let mut line_x = x;
    let line_y = icon.min.y + 39.0;
    for text in [d.author, d.downloads.map(|n| tr_args("mod.downloads", &[&mods::short_count(n)]))].into_iter().flatten() {
        let galley = painter.layout_no_wrap(text, theme::regular_font(15.0), detail);
        let w = galley.size().x;
        painter.galley(Align2::LEFT_CENTER.anchor_size(pos2(line_x, line_y), galley.size()).min, galley, detail);
        line_x += w + 18.0;
    }

    if let Some(url) = d.link {
        let label = tr("mod.open_modrinth");
        let galley = painter.layout_no_wrap(label.clone(), theme::text_font(15.0), theme::PLAY);
        let link = Align2::LEFT_CENTER.anchor_size(pos2(x, icon.min.y + 63.0), galley.size());
        let response = ui.interact(link, Id::unique("mod_open_modrinth"), Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Link, true, &label));
        let color = if response.hovered() { theme::PLAY.lerp_to_gamma(Color32::WHITE, 0.2) } else { theme::PLAY };
        painter.galley(link.min, galley, color);
        if response.hovered() {
            painter.line_segment([pos2(link.min.x, link.max.y), link.max], Stroke::new(1.5, color));
        }
        if response.has_focus() {
            theme::focus_ring(&painter, link.expand(2.0), 4);
        }
        if response.clicked() {
            shell::open(url);
        }
    }

    let boxed = Rect::from_min_max(pos2(inner.min.x, icon.max.y + 16.0), inner.max);
    let corners = theme::radius(theme::CONTROL_RADIUS);
    painter.rect_filled(boxed, corners, theme::GLASS_TINT);
    theme::selectable_outline(&painter, boxed, corners, 0.0, 0.0, theme::Edge::Solid);
    if let Some(text) = d.description {
        let text_rect = boxed.shrink(theme::DETAIL_PADDING);
        let job = egui::text::LayoutJob::simple(text, theme::regular_font(15.0), theme::TEXT.gamma_multiply(0.85), text_rect.width());
        let galley = painter.layout_job(job);
        painter.with_clip_rect(text_rect.intersect(painter.clip_rect())).galley(text_rect.min, galley, theme::TEXT);
    }
}

/// The profile's Launch button: the gold accent with a play icon; while its game starts,
/// the step it's on with a turning ring; while it runs, a red Stop. Faded while
/// another profile's game is starting or running.
fn launch_button(ui: &mut egui::Ui, rect: Rect, status: &profile_list::Status) -> Option<Action> {
    use profile_list::Status;
    let id = Id::unique("profile_launch");
    let time = ui.input(|i| i.time) as f32;
    let (clicked, action) = match status {
        Status::Idle { can_launch } => {
            let clicked = crate::ui::filled_pill(ui, rect, id, &tr("play"), |p, c, color| profile_list::play_icon(p, c, 0.9, color), theme::PLAY, theme::PLAY_TEXT, *can_launch);
            (clicked, Action::Launch)
        }
        Status::Failed(error) => {
            let clicked = crate::ui::filled_pill(ui, rect, id, &tr("play"), |p, c, color| profile_list::play_icon(p, c, 0.9, color), theme::PLAY, theme::PLAY_TEXT, true);
            ui.interact(rect, id.with("failed"), Sense::hover()).on_hover_text(tr_args("status.failed.details", &[error]));
            (clicked, Action::Launch)
        }
        Status::Working { text, progress } => {
            ui.ctx().request_repaint();
            let spin = |p: &Painter, c: Pos2, color: Color32| profile_list::spinner(p, c, 6.0, *progress, time, color);
            crate::ui::filled_pill(ui, rect, id, text, spin, theme::PLAY, theme::PLAY_TEXT, false);
            (false, Action::None)
        }
        Status::Running => {
            let square = |p: &Painter, c: Pos2, color: Color32| {
                p.rect_filled(Rect::from_center_size(c, egui::Vec2::splat(10.0)), 2.0, color);
            };
            let clicked = crate::ui::filled_pill(ui, rect, id, &tr("status.stop"), square, theme::STOP, Color32::WHITE, true);
            (clicked, Action::Stop)
        }
    };
    clicked.then_some(action)
}

/// The result pages: previous, page numbers (the first, the last and those
/// around the current one, gaps shown as dots) and next. Returns the page picked.
fn pager_ui(ui: &mut egui::Ui, rect: Rect, page: usize, pages: usize) -> Option<usize> {
    if pages <= 1 {
        return None;
    }
    let mut items: Vec<Option<usize>> = Vec::new();
    for p in 0..pages {
        if p == 0 || p + 1 == pages || p.abs_diff(page) <= 2 {
            items.push(Some(p));
        } else if items.last() != Some(&None) {
            items.push(None);
        }
    }
    let size = theme::PAGER_HEIGHT;
    let step = size + 6.0;
    let width = step * (items.len() + 2) as f32 - 6.0;
    let mut x = rect.center().x - width / 2.0;
    let mut slot = || {
        let r = Rect::from_min_size(pos2(x, rect.min.y), egui::Vec2::splat(size));
        x += step;
        r
    };
    let mut picked = None;
    let prev = slot();
    if page > 0 && circle_button(ui, prev, Id::unique("pager_prev"), "mods.page.previous", |p, c, color, _| back_icon(p, c, color)) {
        picked = Some(page - 1);
    }
    if page == 0 {
        back_icon(ui.painter(), prev.center(), theme::TEXT.gamma_multiply(0.3));
    }
    for item in &items {
        let r = slot();
        match item {
            Some(p) => {
                if page_button(ui, r, *p, *p == page) {
                    picked = Some(*p);
                }
            }
            None => {
                ui.painter().text(r.center(), Align2::CENTER_CENTER, "...", theme::regular_font(14.0), theme::TEXT.gamma_multiply(0.5));
            }
        }
    }
    let next = slot();
    let forward = |p: &Painter, c: Pos2, color: Color32| {
        let s = Stroke::new(theme::ICON_STROKE, color);
        p.add(egui::Shape::line(vec![c + vec2(-3.0, -6.0), c + vec2(3.0, 0.0), c + vec2(-3.0, 6.0)], s));
    };
    if page + 1 < pages && circle_button(ui, next, Id::unique("pager_next"), "mods.page.next", |p, c, color, _| forward(p, c, color)) {
        picked = Some(page + 1);
    }
    if page + 1 >= pages {
        forward(ui.painter(), next.center(), theme::TEXT.gamma_multiply(0.3));
    }
    picked
}

/// A page number; the current page has the selection box.
fn page_button(ui: &mut egui::Ui, rect: Rect, page: usize, current: bool) -> bool {
    let id = Id::unique(("pager_page", page));
    let label = (page + 1).to_string();
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, current, &label));
    let pressed = response.is_pointer_button_down_on();
    let drawn = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), id, pressed));
    let painter = ui.painter();
    let r = (rect.height() / 2.0) as u8;
    theme::selector(painter, drawn, r, if current { 1.0 } else { 0.0 });
    if response.hovered() && !current {
        painter.rect_filled(drawn, theme::radius(r), if pressed { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
    }
    if response.has_focus() {
        theme::focus_ring(painter, drawn, r);
    }
    let color = if current { theme::TEXT } else { theme::TEXT.gamma_multiply(0.75) };
    painter.text(drawn.center(), Align2::CENTER_CENTER, label, theme::text_font(14.0), color);
    response.clicked() && !current
}

/// A round glass button with an icon (header buttons, install, pages). The icon
/// is told whether the button is hovered or pressed.
fn circle_button(ui: &mut egui::Ui, rect: Rect, id: Id, name_key: &str, icon: impl FnOnce(&Painter, Pos2, Color32, bool)) -> bool {
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr(name_key)));
    let pressed = response.is_pointer_button_down_on();
    let drawn = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), id, pressed));
    let corners = theme::radius((rect.width() / 2.0) as u8);
    let highlight = theme::selected_amount(ui.ctx(), id, response.hovered() || response.has_focus());
    let painter = ui.painter();
    theme::glass(&painter.with_clip_rect(drawn.expand(24.0)), drawn, corners, 0.0, 40, highlight, theme::Edge::Solid);
    if response.hovered() {
        painter.rect_filled(drawn, corners, if pressed { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
    }
    if response.has_focus() {
        theme::focus_ring(painter, drawn, (rect.width() / 2.0) as u8);
    }
    icon(painter, drawn.center(), theme::TEXT, response.hovered() || pressed);
    response.clicked()
}

/// An on/off switch: the gold accent with the knob right when on.
fn toggle_ui(ui: &mut egui::Ui, rect: Rect, id: Id, on: bool) -> bool {
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, tr("mod.enable")));
    let t = ui.ctx().animate_bool_with_time(id, on, 0.12);
    let painter = ui.painter();
    let corners = theme::radius((rect.height() / 2.0) as u8);
    let off = if response.hovered() { theme::GLASS_PRESS } else { theme::GLASS_HOVER };
    painter.rect_filled(rect, corners, off.lerp_to_gamma(theme::PLAY, t));
    let r = rect.height() / 2.0 - 3.0;
    let x = egui::lerp(rect.min.x + 3.0 + r..=rect.max.x - 3.0 - r, t);
    painter.circle_filled(pos2(x, rect.center().y), r, Color32::WHITE);
    if response.has_focus() {
        theme::focus_ring(painter, rect, (rect.height() / 2.0) as u8);
    }
    response.clicked()
}

/// Remove: a bin; after one click it turns red until clicked again (removes) or it times out.
fn remove_button(ui: &mut egui::Ui, rect: Rect, id: Id, armed: bool) -> bool {
    let response = ui.interact(rect, id, Sense::click()).on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, tr("mod.remove")));
    let pressed = response.is_pointer_button_down_on();
    let drawn = theme::scale_about(rect, rect.center(), theme::press_scale(ui.ctx(), id, pressed));
    let painter = ui.painter();
    let corners = theme::radius(theme::SMALL_RADIUS);
    if armed {
        painter.rect_filled(drawn, corners, if pressed { theme::DANGER.lerp_to_gamma(theme::BG, 0.2) } else { theme::DANGER });
    } else if response.hovered() {
        painter.rect_filled(drawn, corners, if pressed { theme::GLASS_PRESS } else { theme::GLASS_HOVER });
    }
    if response.has_focus() {
        theme::focus_ring(painter, drawn, theme::SMALL_RADIUS);
    }
    bin_icon(painter, drawn.center(), if armed { Color32::WHITE } else { theme::TEXT.gamma_multiply(0.8) });
    let response = if armed { response.on_hover_text(tr("mod.remove.confirm")) } else { response };
    response.clicked()
}

// Placeholder icons, drawn with the app's icon stroke until there are real ones.

fn search_icon(p: &Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(2.0, color);
    p.circle_stroke(c + vec2(-1.5, -1.5), 5.5, s);
    p.line_segment([c + vec2(2.5, 2.5), c + vec2(7.0, 7.0)], s);
}

fn back_icon(p: &Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(theme::ICON_STROKE, color);
    p.add(egui::Shape::line(vec![c + vec2(3.0, -6.0), c + vec2(-3.0, 0.0), c + vec2(3.0, 6.0)], s));
}

fn check_icon(p: &Painter, c: Pos2, color: Color32) {
    let s = Stroke::new(theme::ICON_STROKE, color);
    p.add(egui::Shape::line(vec![c + vec2(-6.0, 0.5), c + vec2(-2.0, 4.5), c + vec2(6.5, -4.5)], s));
}

/// A rounded bin (owner request), laid out on whole pixels around a centre
/// snapped to the pixel grid, so its walls and the two bars inside are even and
/// centred: 2 px walls, bars and gaps.
fn bin_icon(p: &Painter, c: Pos2, color: Color32) {
    use egui::emath::GuiRounding;
    let c = c.round_to_pixels(p.ctx().pixels_per_point());
    let pill = |x0: f32, y0: f32, x1: f32, y1: f32| p.rect_filled(Rect::from_min_max(c + vec2(x0, y0), c + vec2(x1, y1)), 1.0, color);
    // Handle (rounded on top) and lid.
    p.rect_filled(Rect::from_min_max(c + vec2(-3.0, -9.0), c + vec2(3.0, -6.0)), egui::CornerRadius { nw: 2, ne: 2, sw: 0, se: 0 }, color);
    pill(-9.0, -6.0, 9.0, -4.0);
    // Body: an open U with round bottom corners (stroke centred on x = ±6, y = 7).
    let r = 3.5;
    let mut body = vec![c + vec2(-6.0, -4.0)];
    let arc = |center: Pos2, from: f32, to: f32| {
        (0..=8).map(move |k| {
            let a = from + (to - from) * k as f32 / 8.0;
            center + vec2(a.cos(), a.sin()) * r
        })
    };
    use std::f32::consts::{FRAC_PI_2, PI};
    body.extend(arc(c + vec2(-6.0 + r, 7.0 - r), PI, FRAC_PI_2));
    body.extend(arc(c + vec2(6.0 - r, 7.0 - r), FRAC_PI_2, 0.0));
    body.push(c + vec2(6.0, -4.0));
    p.add(egui::Shape::line(body, Stroke::new(2.0, color)));
    // Two bars inside, rounded.
    pill(-3.0, -1.0, -1.0, 4.0);
    pill(1.0, -1.0, 3.0, 4.0);
}
