//! The Reawakening OS interface.
//!
//! One `App` holds all interface state and draws every surface. Surfaces are
//! plain pixel buffers plus a list of hit regions, so the same code renders to
//! a Wayland surface in a live session and to a PNG in a unit test.

pub mod bar;
pub mod deskicons;
pub mod dialog;
pub mod launcher;
pub mod quick;
pub mod settings;
pub mod widgets;

use tiny_skia::{Color, Pixmap, Rect};

use crate::apps::{self, App as DesktopApp};
use crate::system::{BtDevice, BtStatus, PowerAction, WifiNetwork, WifiSec};
use crate::text::TextEngine;
use crate::{icons, render, theme, wallpaper};

pub use widgets::Ctx;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// Full-output wallpaper layer, drawn once per output-size change.
    Desktop,
    Bar,
    /// The same wallpaper and bar, on a second or further display. The number is
    /// the index of the output in the order the compositor advertised them.
    DesktopOn(u32),
    BarOn(u32),
    Launcher,
    Quick,
    /// The power confirmation: a full-screen modal that holds the keyboard.
    Dialog,
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowKind {
    Settings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Updates,
    Display,
    Appearance,
    Sound,
    Power,
    About,
}

impl Section {
    pub const ALL: [Section; 6] = [
        Section::Updates,
        Section::Display,
        Section::Appearance,
        Section::Sound,
        Section::Power,
        Section::About,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Section::Updates => "Updates",
            Section::Display => "Display",
            Section::Appearance => "Appearance",
            Section::Sound => "Sound",
            Section::Power => "Power",
            Section::About => "About",
        }
    }

    pub fn glyph(self) -> icons::Glyph {
        match self {
            Section::Updates => icons::Glyph::Refresh,
            Section::Display => icons::Glyph::Display,
            Section::Appearance => icons::Glyph::Moon,
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

/// A part of the system that an established program handles better than this
/// shell can: the full NetworkManager, BlueZ and PipeWire interfaces.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    WifiSettings,
    BluetoothDevices,
    SoundSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    Nothing,
    LauncherOpen,
    LauncherClose,
    LauncherChar(char),
    LauncherBackspace,
    Launch(usize),
    /// A taskbar icon: focus the running window, or start the application.
    LaunchPinned(usize),
    /// A click on a desktop icon (selects it, or opens it on a double click).
    DeskIcon(usize),
    /// Right-click on a desktop icon.
    DeskMenu(usize),
    /// Right-click on an empty part of the desktop.
    DeskMenuEmpty,
    DeskMenuClose,
    DeskMenuShortcuts,
    DeskShortcut(usize),
    DeskOpen,
    DeskRename,
    DeskDelete,
    DeskNewFolder,
    DeskRefresh,
    DeskRenameChar(char),
    DeskRenameBackspace,
    DeskRenameCommit,
    DeskRenameCancel,
    ActivateToplevel(usize),
    SearchWeb,
    QuickToggle,
    QuickClose,
    /// Answer the power confirmation without a power action.
    DialogCancel,
    /// Carry out the power action the confirmation is asking about.
    DialogConfirm,
    /// Move the keyboard focus between Cancel and the destructive button.
    DialogFocus(bool),
    WifiPower,
    WifiList,
    WifiRescan,
    WifiPick(usize),
    WifiPasswordChar(char),
    WifiPasswordBackspace,
    WifiPasswordSubmit,
    /// Give the password field the keyboard focus (a click on the field).
    WifiPasswordFocus,
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
    /// Hand a job to a proven program instead of drawing it here.
    OpenTool(Tool),
    Section(Section),
    UpdateCheck,
    UpdateInstall,
    DisplayMode(usize),
    /// Pick one of the two shipped wallpapers, by index in `wallpaper::Choice::ALL`.
    Wallpaper(usize),
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
    /// True only when a backlight can actually be changed on this machine.
    pub backlight: bool,
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
    /// Security method of `pending`, from the scan that listed it.
    pub pending_security: WifiSec,
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

/// Whether the taskbar sits along the bottom edge of the screen.
///
/// The session anchors the bar and the quick panel draws itself beside it, so
/// both have to read this the same way.
pub fn bar_is_bottom() -> bool {
    std::env::var("REFURB_BAR_POSITION")
        .map(|value| !value.trim().eq_ignore_ascii_case("top"))
        .unwrap_or(true)
}

#[derive(Clone, Debug)]
pub struct State {
    pub view: Surface,
    pub launcher_open: bool,
    pub quick_open: bool,
    /// The power action waiting for confirmation, if the dialog is open.
    pub confirm: Option<PowerAction>,
    /// True when the destructive button has the keyboard focus. Cancel is the
    /// default, so a stray Enter can never power the machine off.
    pub confirm_danger: bool,
    pub window: Option<WindowKind>,
    pub query: String,
    pub focus: Focus,
    pub status: Status,
    pub apps: Vec<DesktopApp>,
    pub pinned: Vec<usize>,
    /// Applications that stay on the taskbar while they are closed.
    pub taskbar: Vec<usize>,
    pub web: Vec<usize>,
    pub toplevels: Vec<Toplevel>,
    pub wifi: WifiUi,
    pub bt: BtUi,
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
    /// The wallpaper the session is painting; see `crate::wallpaper`.
    pub wallpaper: wallpaper::Choice,
    /// The files and shortcuts on `~/Desktop` and where they were left.
    pub desk: crate::desktop::State,
    /// "starting", "listening" or an error, while Alt+Z dictation is active.
    pub dictation: Option<String>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            view: Surface::Bar,
            launcher_open: false,
            quick_open: false,
            confirm: None,
            confirm_danger: false,
            window: None,
            query: String::new(),
            focus: Focus::None,
            status: Status::default(),
            apps: Vec::new(),
            pinned: Vec::new(),
            taskbar: Vec::new(),
            web: Vec::new(),
            toplevels: Vec::new(),
            wifi: WifiUi::default(),
            bt: BtUi::default(),
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
            wallpaper: wallpaper::Choice::default(),
            desk: crate::desktop::State::default(),
            dictation: None,
        }
    }
}

/// Read everything the interface shows about the machine. Slow: it launches
/// `nmcli`, `bluetoothctl` and `wpctl`, so the session calls it from a worker
/// thread and never from the event loop.
pub fn read_status() -> Status {
    use crate::system;
    let mut status = Status::default();
    status.wifi_present = system::network_manager_present();
    status.wifi_enabled = system::wifi_enabled();
    status.wifi = system::wifi_status();
    status.bt = system::bt_status();
    let (vol, muted) = system::volume();
    status.volume = vol;
    status.muted = muted;
    status.battery = system::battery();
    status.brightness = system::brightness();
    status.backlight = system::brightness_available();
    status.updates = system::cached_update_count();
    status.hostname = system::hostname();
    if let Some(m) = system::memory() {
        status.mem_total_mb = m.total_mb;
        status.mem_available_mb = m.available_mb;
    }
    status.cpu = system::cpu_model();
    status
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
        app.st.wallpaper = wallpaper::restore();
        app.reload_apps();
        app.refresh_status();
        app.tick();
        app
    }

    pub fn reload_apps(&mut self) {
        self.st.apps = apps::scan();
        self.st.pinned = apps::pinned(&self.st.apps);
        self.st.taskbar = apps::taskbar_pins(&self.st.apps);
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
        let status = read_status();
        self.apply_status(status, false);
    }

    /// Replace the machine status without clobbering a control the user is
    /// moving right now. A poll that lands mid-drag used to snap the slider
    /// back to the level it read before the drag started, which is one of the
    /// reasons the volume and brightness sliders were unusable.
    pub fn apply_status(&mut self, status: Status, keep_controls: bool) {
        let volume = self.st.status.volume;
        let muted = self.st.status.muted;
        let brightness = self.st.status.brightness;
        let backlight = self.st.status.backlight;
        self.st.status = status;
        if keep_controls {
            self.st.status.volume = volume;
            self.st.status.muted = muted;
            if brightness.is_some() {
                self.st.status.brightness = brightness;
                self.st.status.backlight = true;
            } else {
                self.st.status.backlight = backlight;
            }
        }
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
            Surface::Desktop | Surface::DesktopOn(_) => paint_desktop_background(&mut pm),
            Surface::Bar | Surface::BarOn(_) => pm.fill(theme::panel()),
            Surface::Launcher | Surface::Quick | Surface::Dialog => pm.fill(Color::TRANSPARENT),
            Surface::Settings => pm.fill(theme::card()),
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
                // The wallpaper is painted by the fill above. The primary
                // screen also carries the desktop icons; a second screen gets
                // the same wallpaper and no second set of files.
                Surface::Desktop => deskicons::draw(&mut cx, w as f32, h as f32),
                Surface::DesktopOn(_) => {}
                Surface::Bar | Surface::BarOn(_) => bar::draw(&mut cx, w as f32, h as f32),
                Surface::Launcher => launcher::draw(&mut cx, w as f32, h as f32),
                Surface::Quick => quick::draw(&mut cx, w as f32, h as f32),
                Surface::Dialog => dialog::draw(&mut cx, w as f32, h as f32),
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
        if matches!(surface, Surface::Bar | Surface::BarOn(_)) {
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
/// Known applications map onto the hand-drawn Reawakening set instead.
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

/// The desktop background: the shipped wallpaper, or the drawn fallback below.
pub fn desktop_background(w: u32, h: u32) -> Pixmap {
    let mut pm = Pixmap::new(w.max(1), h.max(1)).expect("surface size");
    paint_desktop_background(&mut pm);
    pm
}

/// Paint the wallpaper into an existing pixmap. The live session uses this so
/// the background layer surface allocates exactly one buffer at output size.
pub fn paint_desktop_background(pm: &mut Pixmap) {
    let (w, h) = (pm.width(), pm.height());
    // The shipped wallpaper, decoded once and cropped to this output's own
    // aspect ratio. Every screen is painted with the same picture at its own
    // size, so a second monitor is neither stretched nor left blank.
    if wallpaper::paint(pm) {
        return;
    }
    // Fallback for a machine without the images (a preview build, or a system
    // where the backgrounds were removed): one diagonal paper gradient plus a
    // soft accent glow, about a millisecond to draw at any resolution.
    render::fill_gradient(
        pm,
        render::rect(0.0, 0.0, w as f32, h as f32),
        theme::paper(),
        theme::mix(theme::paper(), theme::accent_soft(), 0.85),
    );
    render::circle(
        pm,
        w as f32 * 0.78,
        h as f32 * 0.12,
        w.min(h) as f32 * 0.55,
        Color::from_rgba8(0x2C, 0x3F, 0x3B, 0x55),
    );
    let mark = (w.min(h) as f32 * 0.22).clamp(60.0, 190.0);
    let area = render::rect(w as f32 - mark - 60.0, h as f32 - mark - 70.0, mark, mark);
    icons::draw(
        pm,
        icons::Glyph::Brand,
        area,
        theme::mix(theme::paper(), theme::accent(), 0.22),
        1.9,
    );
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
    fn the_desktop_draws_icons_and_registers_them() {
        let mut app = App::new();
        app.st.desk.icons = vec![crate::desktop::Icon {
            name: "Brave".into(),
            file: "Brave.desktop".into(),
            path: std::path::PathBuf::from("/home/x/Desktop/Brave.desktop"),
            kind: crate::desktop::Kind::App,
            app: None,
            col: 0,
            row: 0,
        }];
        let (pm, hits) = app.render(Surface::Desktop, 800, 600);
        let painted = pm.pixels().iter().filter(|p| p.alpha() > 0).count();
        assert!(painted > 10_000, "the wallpaper itself should be painted");
        assert!(
            hits.iter().any(|(_, action)| matches!(action, Action::DeskIcon(0))),
            "an icon must be clickable"
        );
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
            Surface::Desktop,
            Surface::Launcher,
            Surface::Quick,
            Surface::Settings,
        ] {
            let (pm, _) = app.render(surface, 1024, 700);
            let painted = pm.pixels().iter().filter(|p| p.alpha() > 0).count();
            assert!(painted > 1000, "{surface:?} rendered almost nothing");
        }
    }
}
