//! The Refurbished Linux interface.
//!
//! One `App` holds all interface state and draws every surface. Surfaces are
//! plain pixel buffers plus a list of hit regions, so the same code renders to
//! a Wayland surface in a live session and to a PNG in a unit test.

pub mod bar;
pub mod launcher;
pub mod quick;
pub mod settings;
pub mod software;
pub mod widgets;

use tiny_skia::{Color, Pixmap, Rect};

use crate::apps::{self, App as DesktopApp};
use crate::system::{BtDevice, BtStatus, PowerAction, WifiNetwork};
use crate::text::TextEngine;
use crate::{icons, render, theme};

pub use widgets::Ctx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Bar,
    Launcher,
    Quick,
    Software,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowKind {
    Software,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Updates,
    Display,
    Sound,
    Power,
    About,
}

impl Section {
    pub const ALL: [Section; 5] = [
        Section::Updates,
        Section::Display,
        Section::Sound,
        Section::Power,
        Section::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Updates => "Updates",
            Section::Display => "Display",
            Section::Sound => "Sound",
            Section::Power => "Power",
            Section::About => "About",
        }
    }

    pub fn glyph(self) -> icons::Glyph {
        match self {
            Section::Updates => icons::Glyph::Refresh,
            Section::Display => icons::Glyph::Display,
            Section::Sound => icons::Glyph::Volume,
            Section::Power => icons::Glyph::Power,
            Section::About => icons::Glyph::Shield,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Focus {
    None,
    Launcher,
    Software,
    WifiPassword,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SliderKind {
    Volume,
    Brightness,
}

/// A window known to the compositor, used by the taskbar.
#[derive(Clone, Debug)]
pub struct Toplevel {
    pub handle: usize,
    pub title: String,
    pub app_id: String,
    pub active: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Nothing,
    LauncherOpen,
    LauncherClose,
    LauncherChar(char),
    LauncherBackspace,
    Launch(usize),
    ActivateToplevel(usize),
    SearchWeb,
    QuickToggle,
    QuickClose,
    WifiPower,
    WifiList,
    WifiRescan,
    WifiPick(usize),
    WifiPasswordChar(char),
    WifiPasswordBackspace,
    WifiPasswordSubmit,
    WifiForget,
    BtPower,
    BtList,
    BtRescan,
    BtPick(usize),
    BtToggleDevice(usize),
    VolumeSet(f32),
    BrightnessSet(f32),
    MuteToggle,
    WindowOpen(WindowKind),
    WindowClose(WindowKind),
    SoftwareChar(char),
    SoftwareBackspace,
    SoftwarePick(usize),
    SoftwareSuggest(String),
    SoftwareAct,
    SoftwareScroll(i32),
    Section(Section),
    UpdateCheck,
    UpdateInstall,
    DisplayMode(usize),
    Power(PowerAction),
    Lock,
    Quit,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub wifi: Option<crate::system::WifiStatus>,
    pub wifi_enabled: bool,
    pub wifi_present: bool,
    pub bt: BtStatus,
    pub volume: f32,
    pub muted: bool,
    pub battery: Option<(f32, bool)>,
    pub brightness: Option<f32>,
    pub updates: u32,
    pub hostname: String,
    pub mem_total_mb: u64,
    pub mem_available_mb: u64,
    pub cpu: String,
}

#[derive(Clone, Debug, Default)]
pub struct WifiUi {
    pub open: bool,
    pub networks: Vec<WifiNetwork>,
    pub pending: Option<String>,
    pub password: String,
    pub error: Option<String>,
    pub scanning: bool,
}

#[derive(Clone, Debug, Default)]
pub struct BtUi {
    pub open: bool,
    pub devices: Vec<BtDevice>,
    pub scanning: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Pkg {
    pub name: String,
    pub summary: String,
    pub installed: bool,
}

#[derive(Clone, Debug, Default)]
pub struct SoftwareUi {
    pub query: String,
    pub results: Vec<Pkg>,
    pub selected: Option<usize>,
    pub busy: bool,
    pub log: Vec<String>,
    pub scroll: usize,
    pub searched: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayOutput {
    pub name: String,
    pub modes: Vec<String>,
    pub current: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct DisplayUi {
    pub outputs: Vec<DisplayOutput>,
    pub selected: usize,
}

#[derive(Clone, Debug, Default)]
pub struct SettingsUi {
    pub section: Section,
}

impl Default for Section {
    fn default() -> Self {
        Section::Updates
    }
}

#[derive(Clone, Debug)]
pub struct State {
    pub view: Surface,
    pub launcher_open: bool,
    pub quick_open: bool,
    pub window: Option<WindowKind>,
    pub query: String,
    pub focus: Focus,
    pub status: Status,
    pub apps: Vec<DesktopApp>,
    pub pinned: Vec<usize>,
    pub web: Vec<usize>,
    pub toplevels: Vec<Toplevel>,
    pub wifi: WifiUi,
    pub bt: BtUi,
    pub software: SoftwareUi,
    pub settings: SettingsUi,
    pub display: DisplayUi,
    pub clock: String,
    pub weekday: String,
    pub today: String,
    pub hover: Option<(f32, f32)>,
    pub drag: Option<SliderKind>,
    pub slider_volume: Rect,
    pub slider_brightness: Rect,
    pub updates_note: Vec<String>,
    pub version: String,
}

impl Default for State {
    fn default() -> Self {
        Self {
            view: Surface::Bar,
            launcher_open: false,
            quick_open: false,
            window: None,
            query: String::new(),
            focus: Focus::None,
            status: Status::default(),
            apps: Vec::new(),
            pinned: Vec::new(),
            web: Vec::new(),
            toplevels: Vec::new(),
            wifi: WifiUi::default(),
            bt: BtUi::default(),
            software: SoftwareUi::default(),
            settings: SettingsUi::default(),
            display: DisplayUi::default(),
            clock: "--:--".into(),
            weekday: String::new(),
            today: String::new(),
            hover: None,
            drag: None,
            slider_volume: render::rect(0.0, 0.0, 0.0, 0.0),
            slider_brightness: render::rect(0.0, 0.0, 0.0, 0.0),
            updates_note: Vec::new(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

pub struct App {
    pub st: State,
    pub text: TextEngine,
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            st: State::default(),
            text: TextEngine::new(),
        };
        app.reload_apps();
        app.refresh_status();
        app.tick();
        app
    }

    pub fn reload_apps(&mut self) {
        self.st.apps = apps::scan();
        self.st.pinned = apps::pinned(&self.st.apps);
        self.st.web = self
            .st
            .apps
            .iter()
            .enumerate()
            .filter(|(_, a)| a.is_web())
            .map(|(i, _)| i)
            .collect();
    }

    /// Values that change slowly. Called at session start and on a 30s timer.
    pub fn refresh_status(&mut self) {
        use crate::system;
        self.st.status.wifi_present = system::network_manager_present();
        self.st.status.wifi_enabled = system::wifi_enabled();
        self.st.status.wifi = system::wifi_status();
        self.st.status.bt = system::bt_status();
        let (vol, muted) = system::volume();
        self.st.status.volume = vol;
        self.st.status.muted = muted;
        self.st.status.battery = system::battery();
        self.st.status.brightness = system::brightness();
        self.st.status.updates = system::cached_update_count();
        self.st.status.hostname = system::hostname();
        if let Some(m) = system::memory() {
            self.st.status.mem_total_mb = m.total_mb;
            self.st.status.mem_available_mb = m.available_mb;
        }
        self.st.status.cpu = system::cpu_model();
    }

    /// Clock text; cheap enough to call every second.
    pub fn tick(&mut self) {
        let (clock, weekday, today) = crackers_clock();
        if self.st.clock != clock {
            self.st.clock = clock;
            self.st.weekday = weekday;
            self.st.today = today;
        }
    }

    /// Launcher results for the current query.
    pub fn launcher_results(&self) -> Vec<usize> {
        apps::search(&self.st.apps, &self.st.query)
    }

    pub fn render(&mut self, surface: Surface, w: u32, h: u32) -> (Pixmap, Vec<(Rect, Action)>) {
        let mut pm = match Pixmap::new(w.max(1), h.max(1)) {
            Some(p) => p,
            None => return (Pixmap::new(1, 1).expect("1x1"), Vec::new()),
        };
        let mut st = std::mem::take(&mut self.st);
        st.view = surface;
        match surface {
            Surface::Bar => pm.fill(theme::panel()),
            Surface::Launcher | Surface::Quick => pm.fill(Color::TRANSPARENT),
            Surface::Software | Surface::Settings => pm.fill(theme::card()),
        }

        let mut hits: Vec<(Rect, Action)> = Vec::new();
        {
            let mut cx = Ctx {
                st: &st,
                text: &mut self.text,
                pm: &mut pm,
                hits: &mut hits,
                hover: st.hover,
            };
            match surface {
                Surface::Bar => bar::draw(&mut cx, w as f32, h as f32),
                Surface::Launcher => launcher::draw(&mut cx, w as f32, h as f32),
                Surface::Quick => quick::draw(&mut cx, w as f32, h as f32),
                Surface::Software => software::draw(&mut cx, w as f32, h as f32),
                Surface::Settings => settings::draw(&mut cx, w as f32, h as f32),
            }
        }
        self.st = st;
        (pm, hits)
    }

    /// Colour a surface with the desktop background (used by the preview tool).
    pub fn render_on_desktop(
        &mut self,
        surface: Surface,
        w: u32,
        h: u32,
    ) -> (Pixmap, Vec<(Rect, Action)>) {
        let (overlay, hits) = self.render(surface, w, h);
        if matches!(surface, Surface::Bar) {
            return (overlay, hits);
        }
        let mut base = desktop_background(w, h);
        base.draw_pixmap(
            0,
            0,
            overlay.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        (base, hits)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// Icon used for an application in the launcher, taskbar and lists.
///
/// The shell deliberately does not read `.desktop` icon names: an icon theme
/// would add a dependency, a cache and a few hundred files to keep updated.
/// Known applications map onto the hand-drawn Refurbished set instead.
pub fn glyph_for(app: &DesktopApp) -> icons::Glyph {
    if app.is_web() {
        // Web shortcuts are told apart by what they are for, not by the fact
        // that they are web pages: four identical globes in the launcher made
        // the Microsoft tiles impossible to scan.
        let t = format!("{} {} {}", app.id.to_lowercase(), app.name.to_lowercase(), app.exec.to_lowercase());
        let has = |k: &str| t.contains(k);
        if has("outlook") || has("mail") {
            return icons::Glyph::Envelope;
        }
        if has("onedrive") || has("drive") || has("cloud") {
            return icons::Glyph::Cloud;
        }
        if has("teams") || has("chat") || has("meet") {
            return icons::Glyph::Chat;
        }
        if has("office") || has("365") || has("word") || has("excel") || has("powerpoint") {
            return icons::Glyph::Doc;
        }
        return icons::Glyph::Globe;
    }
    let s = format!("{} {}", app.id.to_lowercase(), app.name.to_lowercase());
    let has = |k: &str| s.contains(k);
    if has("brave") {
        icons::Glyph::Globe
    } else if has("file") || has("folder") {
        icons::Glyph::Folder
    } else if has("software") || has("install") {
        icons::Glyph::Download
    } else if has("setting") {
        icons::Glyph::Gear
    } else if has("cockpit") {
        icons::Glyph::Shield
    } else if has("terminal") || has("console") || has("shell") {
        icons::Glyph::Terminal
    } else {
        icons::Glyph::Grid
    }
}

/// The desktop background: warm paper with a single large brand watermark.
pub fn desktop_background(w: u32, h: u32) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("surface size");
    // One diagonal paper gradient plus a soft accent glow: no images, no
    // wallpaper daemon, about a millisecond to draw at any resolution.
    render::fill_gradient(
        &mut pm,
        render::rect(0.0, 0.0, w as f32, h as f32),
        theme::paper(),
        theme::mix(theme::paper(), theme::accent_soft(), 0.85),
    );
    render::circle(
        &mut pm,
        w as f32 * 0.78,
        h as f32 * 0.12,
        w.min(h) as f32 * 0.55,
        Color::from_rgba8(0x2C, 0x3F, 0x3B, 0x55),
    );
    let mark = (w.min(h) as f32 * 0.22).clamp(60.0, 190.0);
    let area = render::rect(w as f32 - mark - 60.0, h as f32 - mark - 70.0, mark, mark);
    icons::draw(
        &mut pm,
        icons::Glyph::Brand,
        area,
        theme::mix(theme::paper(), theme::accent(), 0.22),
        1.9,
    );
    pm
}

/// Local wall clock without pulling in a date/time crate: `localtime_r` is in
/// the C library that every Rust binary already links against.
fn crackers_clock() -> (String, String, String) {
    #[repr(C)]
    struct Tm {
        tm_sec: i32,
        tm_min: i32,
        tm_hour: i32,
        tm_mday: i32,
        tm_mon: i32,
        tm_year: i32,
        tm_wday: i32,
        tm_yday: i32,
        tm_isdst: i32,
        tm_gmtoff: i64,
        tm_zone: *const i8,
    }
    extern "C" {
        fn time(t: *mut i64) -> i64;
        fn localtime_r(t: *const i64, tm: *mut Tm) -> *mut Tm;
    }
    let mut now: i64 = 0;
    let mut tm = Tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 1,
        tm_mon: 0,
        tm_year: 70,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    };
    unsafe {
        time(&mut now);
        localtime_r(&now, &mut tm);
    }
    const DAYS: [&str; 7] = [
        "Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday",
    ];
    const MONTHS: [&str; 12] = [
        "January", "February", "March", "April", "May", "June", "July", "August", "September",
        "October", "November", "December",
    ];
    let clock = format!("{:02}:{:02}", tm.tm_hour, tm.tm_min);
    let weekday = DAYS.get(tm.tm_wday as usize % 7).unwrap_or(&"").to_string();
    let today = format!(
        "{} {}",
        DAYS.get(tm.tm_wday as usize % 7).unwrap_or(&""),
        MONTHS.get(tm.tm_mon as usize % 12).unwrap_or(&"January")
    );
    let _ = tm.tm_mday;
    (clock, weekday, today)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_is_formatted() {
        let (c, wd, _) = crackers_clock();
        assert_eq!(c.len(), 5, "HH:MM");
        assert!(c.contains(':'));
        assert!(!wd.is_empty());
    }

    #[test]
    fn every_surface_renders_non_empty() {
        let mut app = App::new();
        app.st.apps = vec![
            DesktopApp {
                id: "brave-browser".into(),
                name: "Brave".into(),
                comment: "Private web browser".into(),
                exec: "brave-browser %U".into(),
                pin: Some(1),
                ..Default::default()
            },
            DesktopApp {
                id: "files".into(),
                name: "Files".into(),
                comment: "Browse your documents".into(),
                exec: "pcmanfm".into(),
                pin: Some(2),
                ..Default::default()
            },
        ];
        app.st.pinned = apps::pinned(&app.st.apps);
        for surface in [
            Surface::Bar,
            Surface::Launcher,
            Surface::Quick,
            Surface::Software,
            Surface::Settings,
        ] {
            let (pm, _) = app.render(surface, 1024, 700);
            let painted = pm.pixels().iter().filter(|p| p.alpha() > 0).count();
            assert!(painted > 1000, "{surface:?} rendered almost nothing");
        }
    }
}
