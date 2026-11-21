//! The Wayland session.
//!
//! One client process, one event queue. It owns a top bar (layer shell), two
//! overlays (launcher and quick settings, also layer shell) and up to two
//! ordinary windows (Software and Settings). Frames are rasterised on the CPU
//! into shared memory buffers: no GPU context, no toolkit, no extra processes.

use std::io::{Read, Write};
use std::os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::os::raw::{c_char, c_int, c_void};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tiny_skia::{Pixmap, Rect};
use wayland_client::globals::{registry_queue_init, GlobalListContents};
use wayland_client::protocol::{
    wl_buffer, wl_compositor, wl_keyboard, wl_output, wl_pointer, wl_registry, wl_seat, wl_shm,
    wl_shm_pool, wl_surface,
};
use wayland_client::{delegate_noop, Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum};
use wayland_protocols::xdg::shell::client::{xdg_surface, xdg_toplevel, xdg_wm_base};
use wayland_protocols_wlr::foreign_toplevel::v1::client::{
    zwlr_foreign_toplevel_handle_v1, zwlr_foreign_toplevel_manager_v1,
};
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

use crate::apps;
use crate::desktop as desk;
use crate::keys::{Key, Keymap, Modifiers};
use crate::system;
use crate::ui::{self, Action, App, Focus, Section, SliderKind, Surface, Tool, WindowKind};
use crate::{pkg, theme};

const BAR_HEIGHT: u32 = theme::BAR_H as u32;

// ---------------------------------------------------------------------------
// libc bits: shared memory files, mapping, poll. Declared directly so the
// shell needs no bindgen, no C toolchain and no extra crate.
// ---------------------------------------------------------------------------

extern "C" {
    fn memfd_create(name: *const c_char, flags: u32) -> c_int;
    fn ftruncate(fd: c_int, len: i64) -> c_int;
    fn mmap(addr: *mut c_void, len: usize, prot: c_int, flags: c_int, fd: c_int, off: i64)
        -> *mut c_void;
    fn munmap(addr: *mut c_void, len: usize) -> c_int;
    fn poll(fds: *mut PollFd, nfds: u64, timeout: c_int) -> c_int;
}

#[repr(C)]
struct PollFd {
    fd: c_int,
    events: i16,
    revents: i16,
}

const POLLIN: i16 = 0x001;
const PROT_READ_WRITE: c_int = 0x1 | 0x2;
const MAP_SHARED: c_int = 0x01;
const MFD_CLOEXEC: u32 = 0x0001;

// ---------------------------------------------------------------------------
// surfaces and buffers
// ---------------------------------------------------------------------------

enum Role {
    Layer(zwlr_layer_surface_v1::ZwlrLayerSurfaceV1),
    Toplevel {
        xdg: xdg_surface::XdgSurface,
        toplevel: xdg_toplevel::XdgToplevel,
    },
}

struct SurfaceEntry {
    id: Surface,
    wl: wl_surface::WlSurface,
    role: Role,
    width: u32,
    height: u32,
    configured: bool,
    buffers: [Option<ShmBuffer>; 2],
    next: usize,
    hits: Vec<(Rect, Action)>,
}

struct ShmBuffer {
    buffer: wl_buffer::WlBuffer,
    _pool: wl_shm_pool::WlShmPool,
    _fd: OwnedFd,
    ptr: *mut u8,
    len: usize,
    width: u32,
    height: u32,
    busy: Arc<AtomicBool>,
}

impl ShmBuffer {
    /// Copy a premultiplied RGBA pixmap into the buffer, converting to the
    /// little-endian ARGB8888 layout that `wl_shm` expects.
    fn write(&self, pm: &Pixmap) {
        let sw = pm.width() as usize;
        let sh = pm.height() as usize;
        let dw = self.width as usize;
        let dh = self.height as usize;
        let src = pm.data();
        for y in 0..sh.min(dh) {
            for x in 0..sw.min(dw) {
                let s = (y * sw + x) * 4;
                let d = (y * dw + x) * 4;
                unsafe {
                    let p = self.ptr.add(d);
                    *p = src[s + 2];
                    *p.add(1) = src[s + 1];
                    *p.add(2) = src[s];
                    *p.add(3) = src[s + 3];
                }
            }
        }
    }
}

impl Drop for ShmBuffer {
    fn drop(&mut self) {
        unsafe {
            munmap(self.ptr as *mut c_void, self.len);
        }
    }
}

struct TlEntry {
    handle: zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
    title: String,
    app_id: String,
    active: bool,
    closed: bool,
}

enum Msg {
    Wifi(Vec<system::WifiNetwork>),
    Connect(Result<(), String>),
    /// Result of bringing up a profile that was already saved, with its SSID so
    /// a missing passphrase can turn back into a password prompt.
    ConnectSaved(String, Result<(), String>),
    Bluetooth(Vec<system::BtDevice>),
    Packages(Vec<ui::Pkg>),
    PackageDone(bool, String),
    Log(String),
    Updates(Result<(u32, u32), String>),
    Displays(Vec<ui::DisplayOutput>),
    /// A machine status snapshot, gathered on a worker thread.
    Status(Box<ui::Status>),
    /// The audio worker read the sink back after writing it, so the panel shows
    /// the level the machine actually has (WirePlumber clamps at 150%) instead
    /// of the position the slider was dragged to.
    VolumeApplied(Result<(f32, bool), String>),
    /// The backlight's real level after the change, or why it did not change.
    BrightnessApplied(Result<f32, String>),
}

pub struct Session {
    conn: Connection,
    qh: QueueHandle<Session>,
    /// The registry snapshot maintained by wayland-client. Kept so the screen
    /// list can be re-read on a timer as well as from registry events.
    globals: wayland_client::globals::GlobalList,
    compositor: wl_compositor::WlCompositor,
    shm: wl_shm::WlShm,
    wm_base: xdg_wm_base::XdgWmBase,
    layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    seat: Option<wl_seat::WlSeat>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    keymap: Keymap,
    modifiers: Modifiers,
    app: App,
    surfaces: Vec<SurfaceEntry>,
    /// Every display the compositor advertised, in registry order. Each one
    /// gets its own wallpaper and its own bar.
    outputs: Vec<wl_output::WlOutput>,
    /// Registry name of every bound output. A monitor that is plugged in or
    /// unplugged while the desktop runs is announced through the registry, and
    /// the name is the only way to match that announcement to a screen.
    output_globals: Vec<(u32, wl_output::WlOutput)>,
    /// Put the bar along the bottom edge instead of the top (REFURB_BAR_POSITION=bottom).
    bar_bottom: bool,
    toplevels: Vec<TlEntry>,
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    pointer_focus: Option<Surface>,
    pointer_pos: (f64, f64),
    drag: Option<(SliderKind, Rect)>,
    search_due: Option<Instant>,
    last_tick: Instant,
    last_status: Instant,
    /// Surfaces whose contents changed and that still need a frame. Painting is
    /// coalesced, so a burst of pointer events costs one frame, not one frame
    /// per event. Redrawing a full-screen wallpaper on every motion event is
    /// what made the desktop feel sluggish on a 2 GB machine.
    dirty: Vec<Surface>,
    last_paint: Instant,
    /// When the user last moved the volume or brightness slider. A status poll
    /// that lands just after must not snap the control back.
    controls_touched: Option<Instant>,
    /// Whether the quick panel currently holds the keyboard focus.
    quick_keys: bool,
    /// The card height the click map was last built for (the surface itself is
    /// full-screen; see `sync_panels`).
    quick_card_height: u32,
    /// Whether the power confirmation currently holds the keyboard focus.
    dialog_keys: bool,
    /// Whether the desktop holds the keyboard because an icon is being renamed.
    desk_keys: bool,
    /// True between a left press and its release: desktop icons are dragged
    /// only while the button is down.
    pointer_down: bool,
    /// What the bar is showing about Alt+Z dictation, and when it last checked.
    dictation: Option<String>,
    last_dictation: Instant,
    /// Hardware writes are coalesced on these worker threads: a drag produces
    /// hundreds of positions but only the newest one is worth applying.
    volume_tx: Sender<f32>,
    brightness_tx: Sender<f32>,
    ipc: Option<UnixListener>,
    exit: bool,
}

pub fn run() -> Result<(), String> {
    install_panic_log();
    let conn = Connection::connect_to_env()
        .map_err(|e| format!("cannot connect to a Wayland compositor: {e}"))?;
    let (globals, mut queue) =
        registry_queue_init::<Session>(&conn).map_err(|e| format!("registry: {e}"))?;
    let qh = queue.handle();

    let compositor: wl_compositor::WlCompositor = globals
        .bind(&qh, 1..=6, ())
        .map_err(|e| format!("wl_compositor unavailable: {e}"))?;
    let shm: wl_shm::WlShm = globals
        .bind(&qh, 1..=1, ())
        .map_err(|e| format!("wl_shm unavailable: {e}"))?;
    let wm_base: xdg_wm_base::XdgWmBase = globals
        .bind(&qh, 1..=6, ())
        .map_err(|e| format!("xdg_wm_base unavailable: {e}"))?;
    let layer_shell = globals.bind(&qh, 1..=4, ()).ok();
    if layer_shell.is_none() {
        eprintln!("refurb-shell: this compositor does not support wlr-layer-shell; the bar cannot be placed");
    }
    let ftl = globals
        .bind::<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, _, _>(&qh, 1..=3, ())
        .ok();

    // Bind every screen. A layer shell surface is placed on one output (or on
    // whichever the compositor prefers when none is given), so the shell needs
    // a proxy per screen to put a bar on each of them.
    let output_globals: Vec<(u32, wl_output::WlOutput)> = {
        let registry = globals.registry().clone();
        globals
            .contents()
            .clone_list()
            .into_iter()
            .filter(|global| global.interface == "wl_output")
            .map(|global| {
                let output = registry.bind::<wl_output::WlOutput, _, _>(
                    global.name,
                    global.version.min(4),
                    &qh,
                    (),
                );
                (global.name, output)
            })
            .collect()
    };
    let outputs: Vec<wl_output::WlOutput> =
        output_globals.iter().map(|(_, o)| o.clone()).collect();

    let (tx, rx) = mpsc::channel();
    let (volume_tx, brightness_tx) = start_control_workers(&tx);
    let now = Instant::now();
    // The desktop exists before the first frame, so a file already in
    // ~/Desktop is on screen as soon as the wallpaper is.
    let mut app = App::new();
    desk::ensure_dir();
    desk::refresh(&mut app.st.desk, true);
    // The taskbar belongs along the bottom edge of every screen, which is what
    // this desktop has been asked for since 0.2. It used to need
    // REFURB_BAR_POSITION=bottom and nothing set it, so every image shipped with
    // the bar at the top. Bottom is the default now; the variable is only an
    // escape hatch for anyone who wants the other edge.
    let bar_bottom = ui::bar_is_bottom();
    // A seat is normally advertised by the registry; bind it through the global
    // list when it has not arrived yet. Done before the globals are moved into
    // the session, which keeps the registry snapshot for later reconciliation.
    let seat = globals.bind::<wl_seat::WlSeat, _, _>(&qh, 1..=7, ()).ok();
    let mut session = Session {
        conn,
        qh: qh.clone(),
        globals,
        compositor,
        shm,
        wm_base,
        layer_shell,
        seat: None,
        keyboard: None,
        pointer: None,
        keymap: Keymap::fallback(),
        modifiers: Modifiers::default(),
        app,
        surfaces: Vec::new(),
        outputs,
        output_globals,
        bar_bottom,
        toplevels: Vec::new(),
        tx,
        rx,
        pointer_focus: None,
        pointer_pos: (0.0, 0.0),
        drag: None,
        search_due: None,
        last_tick: now,
        last_status: now,
        dirty: Vec::new(),
        last_paint: now - Duration::from_millis(20),
        controls_touched: None,
        quick_keys: false,
        quick_card_height: 0,
        dialog_keys: false,
        desk_keys: false,
        pointer_down: false,
        dictation: None,
        last_dictation: now - Duration::from_secs(5),
        volume_tx,
        brightness_tx,
        ipc: start_ipc(),
        exit: false,
    };

    if let Some(seat) = seat {
        session.attach_seat(seat);
    }
    let _ = ftl;

    // Wallpaper first, then the bar: the desktop must be below everything.
    // The first screen keeps the plain ids; every further screen gets its own
    // pair so second and third monitors are not left black and barless.
    session.open_layer(Surface::Desktop);
    session.open_layer(Surface::Bar);
    let extra = session.outputs.len().saturating_sub(1);
    for index in 0..extra {
        let index = index as u32 + 1;
        session.open_layer(Surface::DesktopOn(index));
        session.open_layer(Surface::BarOn(index));
    }
    // REFURB_SHELL_DEBUG=1 explains what the shell put on screen, which is the
    // only way to check multi-monitor placement on a machine with no display.
    if std::env::var_os("REFURB_SHELL_DEBUG").is_some() {
        eprintln!(
            "refurb-shell: {} screen(s) seen by the compositor",
            session.outputs.len()
        );
        for entry in &session.surfaces {
            eprintln!("refurb-shell: surface {:?} {}", entry.id, entry.width);
        }
    }
    session.loop_forever(&mut queue)
}

impl Session {
    // -- seat ------------------------------------------------------------

    fn attach_seat(&mut self, seat: wl_seat::WlSeat) {
        // Input objects are created from the seat's Capabilities event, never
        // here: calling get_keyboard() on a seat that has never advertised a
        // keyboard is a protocol error, and a protocol error tears the whole
        // shell down. The compositor always sends capabilities right after the
        // bind, so nothing is missed by leaving it to the event.
        self.seat = Some(seat);
    }

    // -- surfaces --------------------------------------------------------

    fn entry_index(&self, id: Surface) -> Option<usize> {
        self.surfaces.iter().position(|s| s.id == id)
    }

    fn open_layer(&mut self, id: Surface) {
        if self.entry_index(id).is_some() {
            return;
        }
        let Some(shell) = self.layer_shell.clone() else {
            return;
        };
        let wl = self.compositor.create_surface(&self.qh, id);
        let layer = match id {
            // The wallpaper sits below every window and never takes input.
            Surface::Desktop | Surface::DesktopOn(_) => zwlr_layer_shell_v1::Layer::Background,
            Surface::Bar | Surface::BarOn(_) => zwlr_layer_shell_v1::Layer::Top,
            _ => zwlr_layer_shell_v1::Layer::Overlay,
        };
        // Pin each surface to a real screen: without an output the compositor
        // picks one, which leaves every other monitor without a bar.
        let output = match id {
            Surface::Desktop | Surface::Bar => self.outputs.first().cloned(),
            Surface::DesktopOn(index) | Surface::BarOn(index) => {
                self.outputs.get(index as usize).cloned()
            }
            _ => None,
        };
        let surface = shell.get_layer_surface(
            &wl,
            output.as_ref(),
            layer,
            format!("refurb-shell-{id:?}"),
            &self.qh,
            id,
        );
        let mut width = 0u32;
        let mut height = BAR_HEIGHT;
        let mut exclusive_keys = false;
        match id {
            Surface::Desktop | Surface::DesktopOn(_) => {
                width = 0;
                height = 0;
                // Anchor to all four edges with a zero size: the compositor
                // then tells us the output size in its configure event, which
                // is also how a resolution change reaches the wallpaper.
                surface.set_anchor(
                    zwlr_layer_surface_v1::Anchor::Top
                        | zwlr_layer_surface_v1::Anchor::Bottom
                        | zwlr_layer_surface_v1::Anchor::Left
                        | zwlr_layer_surface_v1::Anchor::Right,
                );
                surface.set_size(0, 0);
                // -1: the wallpaper ignores the bar's exclusive zone and covers
                // the whole output behind it.
                surface.set_exclusive_zone(-1);
                surface.set_keyboard_interactivity(
                    zwlr_layer_surface_v1::KeyboardInteractivity::None,
                );
            }
            Surface::Bar | Surface::BarOn(_) => {
                let edge = if self.bar_bottom {
                    zwlr_layer_surface_v1::Anchor::Bottom
                } else {
                    zwlr_layer_surface_v1::Anchor::Top
                };
                surface.set_anchor(
                    edge | zwlr_layer_surface_v1::Anchor::Left
                        | zwlr_layer_surface_v1::Anchor::Right,
                );
                surface.set_size(0, BAR_HEIGHT);
                surface.set_exclusive_zone(BAR_HEIGHT as i32);
                surface.set_keyboard_interactivity(
                    zwlr_layer_surface_v1::KeyboardInteractivity::None,
                );
            }
            Surface::Launcher => {
                surface.set_anchor(
                    zwlr_layer_surface_v1::Anchor::Top
                        | zwlr_layer_surface_v1::Anchor::Bottom
                        | zwlr_layer_surface_v1::Anchor::Left
                        | zwlr_layer_surface_v1::Anchor::Right,
                );
                surface.set_size(0, 0);
                surface.set_exclusive_zone(0);
                surface.set_keyboard_interactivity(
                    zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
                );
            }
            Surface::Dialog => {
                // Full output, anchored to every edge, with a transparent
                // backdrop: the card sits in the middle and a click outside it
                // is swallowed here instead of reaching the window behind.
                width = 0;
                height = 0;
                surface.set_anchor(
                    zwlr_layer_surface_v1::Anchor::Top
                        | zwlr_layer_surface_v1::Anchor::Bottom
                        | zwlr_layer_surface_v1::Anchor::Left
                        | zwlr_layer_surface_v1::Anchor::Right,
                );
                surface.set_size(0, 0);
                surface.set_exclusive_zone(0);
                surface.set_keyboard_interactivity(
                    zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
                );
                exclusive_keys = true;
            }
            Surface::Quick => {
                // The whole output, with the card drawn in the corner. A small
                // surface never receives a click that lands outside it, so
                // "click anywhere to close" was impossible: the press went to
                // whatever window was underneath and this program never heard
                // about it. Under a full-screen transparent surface those
                // clicks arrive here, and a press that hits no control closes
                // the panel (see on_pointer_button).
                width = 0;
                height = 0;
                surface.set_anchor(
                    zwlr_layer_surface_v1::Anchor::Top
                        | zwlr_layer_surface_v1::Anchor::Bottom
                        | zwlr_layer_surface_v1::Anchor::Left
                        | zwlr_layer_surface_v1::Anchor::Right,
                );
                surface.set_size(0, 0);
                surface.set_exclusive_zone(0);
                // It holds the keyboard while it is open, so Escape closes it.
                // r10 left this at `None`, which is also why the Wi-Fi password
                // field could not be typed into at all: the keys went to the
                // window behind.
                exclusive_keys = true;
                surface.set_keyboard_interactivity(
                    zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
                );
            }
            _ => return,
        }
        wl.commit();
        if matches!(id, Surface::Quick) {
            self.quick_keys = exclusive_keys;
        }
        if matches!(id, Surface::Dialog) {
            self.dialog_keys = exclusive_keys;
        }
        self.note(format!("open {id:?} size {width}x{height}"));
        self.surfaces.push(SurfaceEntry {
            id,
            wl,
            role: Role::Layer(surface),
            width,
            height,
            configured: false,
            buffers: [None, None],
            next: 0,
            hits: Vec::new(),
        });
        // Build the click map before the first frame exists, so an input event
        // that arrives in the same batch as the one that opened this surface is
        // still matched against the layout the user is about to see.
        if width > 0 && height > 0 {
            self.refresh_hits(id, width, height);
        }
    }

    fn open_window(&mut self, kind: WindowKind) {
        let id = surface_for(kind);
        if self.entry_index(id).is_some() {
            return;
        }
        let wl = self.compositor.create_surface(&self.qh, id);
        let xdg = self.wm_base.get_xdg_surface(&wl, &self.qh, id);
        let toplevel = xdg.get_toplevel(&self.qh, id);
        let (title, app_id) = match kind {
            WindowKind::Software => ("Software", "refurb-software"),
            WindowKind::Settings => ("Settings", "refurb-settings"),
        };
        toplevel.set_title(title.to_string());
        toplevel.set_app_id(app_id.to_string());
        toplevel.set_min_size(600, 400);
        wl.commit();
        self.surfaces.push(SurfaceEntry {
            id,
            wl,
            role: Role::Toplevel { xdg, toplevel },
            width: 960,
            height: 620,
            configured: false,
            buffers: [None, None],
            next: 0,
            hits: Vec::new(),
        });
    }

    fn close_surface(&mut self, id: Surface) {
        let Some(idx) = self.entry_index(id) else {
            return;
        };
        let entry = self.surfaces.remove(idx);
        match entry.role {
            Role::Layer(surface) => {
                surface.destroy();
                entry.wl.destroy();
            }
            Role::Toplevel { xdg, toplevel } => {
                toplevel.destroy();
                xdg.destroy();
                entry.wl.destroy();
            }
        }
    }

    fn sync_panels(&mut self) {
        if self.app.st.launcher_open {
            self.open_layer(Surface::Launcher);
        } else {
            self.close_surface(Surface::Launcher);
        }

        if self.app.st.quick_open {
            match self.entry_index(Surface::Quick) {
                Some(idx) => {
                    // The surface covers the whole output now, so expanding a
                    // section only moves the card's top edge and the compositor
                    // never reconfigures anything. The click map has to follow
                    // it by hand, or a click in the part that only exists after
                    // the expansion is matched against the old map and silently
                    // does nothing - the "quick settings ignores clicks" report.
                    let card_height = ui::quick::height(&self.app.st);
                    if self.quick_card_height != card_height {
                        self.quick_card_height = card_height;
                        let (w, h) = (self.surfaces[idx].width, self.surfaces[idx].height);
                        if w > 0 && h > 0 {
                            self.refresh_hits(Surface::Quick, w, h);
                            self.redraw(Surface::Quick);
                        }
                    }
                    // The panel holds the keyboard while it is open, so Escape
                    // reaches it even when the pointer is over an application.
                    if !self.quick_keys {
                        if let Role::Layer(surface) = &self.surfaces[idx].role {
                            surface.set_keyboard_interactivity(
                                zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
                            );
                            self.surfaces[idx].wl.commit();
                        }
                        self.quick_keys = true;
                        self.note("quick panel keyboard focus: true".to_string());
                    }
                    self.redraw(Surface::Quick);
                }
                None => self.open_layer(Surface::Quick),
            }
        } else {
            self.quick_keys = false;
            self.quick_card_height = 0;
            self.close_surface(Surface::Quick);
        }

        if self.app.st.confirm.is_some() {
            match self.entry_index(Surface::Dialog) {
                Some(idx) => {
                    // The dialog keeps the keyboard while it is open, so Escape
                    // reaches it even when the pointer is over an application.
                    if !self.dialog_keys {
                        if let Role::Layer(surface) = &self.surfaces[idx].role {
                            surface.set_keyboard_interactivity(
                                zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive,
                            );
                            self.surfaces[idx].wl.commit();
                        }
                        self.dialog_keys = true;
                    }
                    self.redraw(Surface::Dialog);
                }
                None => self.open_layer(Surface::Dialog),
            }
        } else {
            self.dialog_keys = false;
            self.close_surface(Surface::Dialog);
        }

        // An icon can only be renamed if the keyboard reaches the desktop. The
        // wallpaper layer normally takes no keyboard at all, so renaming is the
        // one moment it asks for it, and it gives it back straight afterwards.
        let desk_keys = self.app.st.desk.renaming.is_some();
        if desk_keys != self.desk_keys {
            if let Some(idx) = self.entry_index(Surface::Desktop) {
                if let Role::Layer(surface) = &self.surfaces[idx].role {
                    surface.set_keyboard_interactivity(if desk_keys {
                        zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive
                    } else {
                        zwlr_layer_surface_v1::KeyboardInteractivity::None
                    });
                    self.surfaces[idx].wl.commit();
                }
            }
            self.desk_keys = desk_keys;
        }

        for kind in [WindowKind::Software, WindowKind::Settings] {
            let id = surface_for(kind);
            let wanted = self.app.st.window == Some(kind);
            let present = self.entry_index(id).is_some();
            if wanted && !present {
                self.open_window(kind);
            } else if !wanted && present {
                self.close_surface(id);
            }
        }
    }

    // -- drawing ---------------------------------------------------------

    fn create_buffer(&self, width: u32, height: u32) -> Option<ShmBuffer> {
        let stride = width as i32 * 4;
        let len = stride as usize * height as usize;
        if len == 0 {
            return None;
        }
        unsafe {
            let name = std::ffi::CString::new("refurb-shell").ok()?;
            let fd = memfd_create(name.as_ptr(), MFD_CLOEXEC);
            if fd < 0 {
                return None;
            }
            let owned = OwnedFd::from_raw_fd(fd);
            if ftruncate(owned.as_raw_fd(), len as i64) != 0 {
                return None;
            }
            let ptr = mmap(
                std::ptr::null_mut(),
                len,
                PROT_READ_WRITE,
                MAP_SHARED,
                owned.as_raw_fd(),
                0,
            );
            if ptr as isize == -1 {
                return None;
            }
            let pool = self.shm.create_pool(owned.as_fd(), len as i32, &self.qh, ());
            let busy = Arc::new(AtomicBool::new(false));
            let buffer = pool.create_buffer(
                0,
                width as i32,
                height as i32,
                stride,
                wl_shm::Format::Argb8888,
                &self.qh,
                busy.clone(),
            );
            Some(ShmBuffer {
                buffer,
                _pool: pool,
                _fd: owned,
                ptr: ptr as *mut u8,
                len,
                width,
                height,
                busy,
            })
        }
    }

    /// Render and commit one surface. Returns false if the frame was dropped
    /// because both buffers were still in use by the compositor.
    fn redraw(&mut self, id: Surface) -> bool {
        let Some(idx) = self.entry_index(id) else {
            return false;
        };
        if !self.surfaces[idx].configured {
            return false;
        }
        let (width, height) = (self.surfaces[idx].width, self.surfaces[idx].height);
        if width == 0 || height == 0 {
            return false;
        }
        let (pm, hits) = self.app.render(id, width, height);
        self.surfaces[idx].hits = hits;

        let slot = [0usize, 1]
            .into_iter()
            .find(|&i| match &self.surfaces[idx].buffers[i] {
                None => true,
                Some(buffer) => !buffer.busy.load(Ordering::Relaxed),
            });
        let Some(slot) = slot else {
            return false;
        };
        let need_new = match &self.surfaces[idx].buffers[slot] {
            Some(buffer) => buffer.width != width || buffer.height != height,
            None => true,
        };
        let fresh = if need_new {
            self.create_buffer(width, height)
        } else {
            None
        };

        let entry = &mut self.surfaces[idx];
        if fresh.is_some() {
            entry.buffers[slot] = fresh;
        }
        let Some(buffer) = &mut entry.buffers[slot] else {
            return false;
        };
        buffer.write(&pm);
        buffer.busy.store(true, Ordering::Relaxed);
        entry.wl.attach(Some(&buffer.buffer), 0, 0);
        entry
            .wl
            .damage(0, 0, width as i32, height as i32);
        entry.wl.commit();
        true
    }

    /// Recompute the click map for a surface without committing a frame.
    ///
    /// A layer surface may not take a buffer of a new size until the compositor
    /// has configured it, which leaves a window where the panel looks expanded
    /// but its hit map still describes the old layout. Rendering only the hit
    /// map costs a few milliseconds and needs no compositor round trip, so
    /// clicks are never matched against a stale layout.
    fn refresh_hits(&mut self, id: Surface, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        if self.entry_index(id).is_none() {
            return;
        }
        let (_, hits) = self.app.render(id, width, height);
        if let Some(idx) = self.entry_index(id) {
            self.surfaces[idx].hits = hits;
        }
    }

    /// Bring the screen list in line with the compositor's registry.
    ///
    /// New outputs are bound and unplugged ones dropped; any change rebuilds the
    /// per-screen wallpaper and bar. Idempotent, so it is safe on a timer.
    fn reconcile_outputs(&mut self) {
        let present: Vec<(u32, u32)> = self
            .globals
            .contents()
            .clone_list()
            .into_iter()
            .filter(|global| global.interface == "wl_output")
            .map(|global| (global.name, global.version))
            .collect();
        if present.len() == self.output_globals.len()
            && present
                .iter()
                .all(|(name, _)| self.output_globals.iter().any(|(n, _)| n == name))
        {
            return;
        }
        self.output_globals
            .retain(|(name, _)| present.iter().any(|(n, _)| n == name));
        for (name, version) in present.iter() {
            if self.output_globals.iter().any(|(n, _)| n == name) {
                continue;
            }
            let registry = self.globals.registry().clone();
            let output =
                registry.bind::<wl_output::WlOutput, _, _>(*name, (*version).min(4), &self.qh, ());
            self.output_globals.push((*name, output));
        }
        self.outputs = self
            .output_globals
            .iter()
            .map(|(_, output)| output.clone())
            .collect();
        self.note(format!(
            "{} screen(s) after reconciling the registry",
            self.outputs.len()
        ));
        self.rebuild_screens();
        self.redraw_visible();
    }

    /// Rebuild the wallpaper and bar of every screen from the registry's
    /// current output list. Only monitors being added or removed reaches this,
    /// so the cost of recreating a handful of layer surfaces is irrelevant.
    fn rebuild_screens(&mut self) {
        let stale: Vec<Surface> = self
            .surfaces
            .iter()
            .filter_map(|entry| match entry.id {
                Surface::Desktop
                | Surface::Bar
                | Surface::DesktopOn(_)
                | Surface::BarOn(_) => Some(entry.id),
                _ => None,
            })
            .collect();
        for id in stale {
            self.close_surface(id);
        }
        if self.outputs.is_empty() {
            self.redraw_visible();
            return;
        }
        self.open_layer(Surface::Desktop);
        self.open_layer(Surface::Bar);
        let extra = self.outputs.len().saturating_sub(1);
        for index in 0..extra {
            let index = index as u32 + 1;
            self.open_layer(Surface::DesktopOn(index));
            self.open_layer(Surface::BarOn(index));
        }
        self.sync_panels();
        self.redraw_visible();
    }

    /// Repaint the bar on every screen, not just the first one.
    fn redraw_bars(&mut self) {
        self.redraw(Surface::Bar);
        let extra: Vec<u32> = self
            .surfaces
            .iter()
            .filter_map(|entry| match entry.id {
                Surface::BarOn(index) => Some(index),
                _ => None,
            })
            .collect();
        for index in extra {
            self.redraw(Surface::BarOn(index));
        }
    }

    fn redraw_visible(&mut self) {
        self.invalidate(None);
    }

    // -- actions ---------------------------------------------------------

    fn note(&self, message: impl AsRef<str>) {
        if std::env::var_os("REFURB_SHELL_DEBUG").is_none() {
            return;
        }
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open("/tmp/refurb-debug.log")
        {
            let _ = writeln!(file, "{}", message.as_ref());
        }
    }

    fn dispatch(&mut self, action: Action) {
        self.note(format!("dispatch {action:?}"));
        // One failed action must never take the desktop down with it: the shell
        // owns the bar and every window, so a panic here looked like "the
        // desktop crashed" until the supervisor restarted it.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.on_action(action.clone());
        }));
        if outcome.is_err() {
            eprintln!("refurb-shell: recovered from a failed action: {action:?}");
        }
        self.sync_panels();
        self.invalidate(None);
    }

    /// True while the quick panel is asking for something that must be typed.
    fn editing_secret(&self) -> bool {
        self.app.st.quick_open && self.app.st.wifi.pending.is_some()
    }

    /// Mark a surface (or every surface) for the next frame.
    fn invalidate(&mut self, id: Option<Surface>) {
        match id {
            Some(surface) => {
                if self.entry_index(surface).is_some() && !self.dirty.contains(&surface) {
                    self.dirty.push(surface);
                }
            }
            None => {
                for entry in &self.surfaces {
                    if !self.dirty.contains(&entry.id) {
                        self.dirty.push(entry.id);
                    }
                }
            }
        }
    }

    /// Paint the surfaces that changed, at most once per frame interval.
    fn flush_dirty(&mut self) {
        if self.dirty.is_empty() {
            return;
        }
        let now = Instant::now();
        if now.duration_since(self.last_paint) < Duration::from_millis(14) {
            // Too soon: `poll_timeout` wakes the loop when the frame is due.
            return;
        }
        self.last_paint = now;
        let ids = std::mem::take(&mut self.dirty);
        for id in ids {
            self.redraw(id);
        }
    }

    fn on_action(&mut self, action: Action) {
        match action {
            Action::Nothing => {}
            Action::LauncherOpen => {
                self.app.st.launcher_open = true;
                self.app.st.quick_open = false;
                self.app.st.focus = Focus::Launcher;
                self.app.st.query.clear();
            }
            Action::LauncherClose => {
                self.app.st.launcher_open = false;
                self.app.st.focus = Focus::None;
            }
            Action::LauncherChar(c) => {
                if self.app.st.query.chars().count() < 64 {
                    self.app.st.query.push(c);
                }
            }
            Action::LauncherBackspace => {
                self.app.st.query.pop();
            }
            Action::Launch(i) => {
                if let Some(app) = self.app.st.apps.get(i) {
                    if let Err(e) = app.launch() {
                        eprintln!("refurb-shell: could not launch {}: {e}", app.name);
                    }
                }
                self.app.st.launcher_open = false;
                self.app.st.focus = Focus::None;
            }
            Action::SearchWeb => {
                let query = self.app.st.query.trim().to_string();
                if !query.is_empty() {
                    let url = format!(
                        "https://search.brave.com/search?q={}",
                        url_encode(&query)
                    );
                    system::spawn("brave-browser", &["--new-window", &url]);
                }
                self.app.st.launcher_open = false;
                self.app.st.focus = Focus::None;
            }
            Action::ActivateToplevel(i) => {
                if let (Some(entry), Some(seat)) = (self.toplevels.get(i), self.seat.as_ref()) {
                    entry.handle.activate(seat);
                }
            }
            Action::LaunchPinned(index) => self.launch_pinned(index),
            Action::DeskIcon(index) => self.desk_press(index),
            Action::DeskMenu(index) => {
                self.app.st.desk.selected = Some(index);
                self.app.st.desk.menu = Some(desk::Menu {
                    icon: Some(index),
                    shortcuts: false,
                    x: self.pointer_pos.0 as f32,
                    y: self.pointer_pos.1 as f32,
                });
            }
            Action::DeskMenuEmpty => {
                self.app.st.desk.selected = None;
                self.app.st.desk.menu = Some(desk::Menu {
                    icon: None,
                    shortcuts: false,
                    x: self.pointer_pos.0 as f32,
                    y: self.pointer_pos.1 as f32,
                });
            }
            Action::DeskMenuClose => self.app.st.desk.menu = None,
            Action::DeskMenuShortcuts => {
                if let Some(menu) = &mut self.app.st.desk.menu {
                    menu.shortcuts = true;
                }
            }
            Action::DeskShortcut(index) => {
                self.app.st.desk.menu = None;
                let Some(app) = self.app.st.apps.get(index).cloned() else {
                    return;
                };
                match desk::install_shortcut(&app) {
                    Ok(name) => {
                        desk::refresh(&mut self.app.st.desk, true);
                        self.app.st.desk.say(format!("Added {name} to the desktop"));
                    }
                    Err(error) => self.app.st.desk.say(error),
                }
            }
            Action::DeskOpen => {
                self.app.st.desk.menu = None;
                self.open_desktop_icon();
            }
            Action::DeskRename => {
                if let Some(index) = self.app.st.desk.selected {
                    if let Some(icon) = self.app.st.desk.icons.get(index) {
                        self.app.st.desk.renaming = Some((index, icon.file.clone()));
                    }
                }
                self.app.st.desk.menu = None;
            }
            Action::DeskRenameChar(c) => {
                if let Some((_, text)) = &mut self.app.st.desk.renaming {
                    if !c.is_control() && text.chars().count() < 120 {
                        text.push(c);
                    }
                }
            }
            Action::DeskRenameBackspace => {
                if let Some((_, text)) = &mut self.app.st.desk.renaming {
                    text.pop();
                }
            }
            Action::DeskRenameCommit => self.commit_rename(),
            Action::DeskRenameCancel => self.app.st.desk.renaming = None,
            Action::DeskDelete => {
                self.app.st.desk.menu = None;
                if let Some(index) = self.app.st.desk.selected {
                    if let Some(icon) = self.app.st.desk.icons.get(index).cloned() {
                        match desk::trash(&icon) {
                            Ok(()) => {
                                self.app.st.desk.say(format!("{} moved to Trash", icon.name));
                                desk::refresh(&mut self.app.st.desk, true);
                            }
                            Err(error) => self.app.st.desk.say(error),
                        }
                    }
                }
            }
            Action::DeskNewFolder => {
                self.app.st.desk.menu = None;
                match desk::new_folder() {
                    Ok(name) => {
                        desk::refresh(&mut self.app.st.desk, true);
                        self.app.st.desk.selected = self
                            .app
                            .st
                            .desk
                            .icons
                            .iter()
                            .position(|icon| icon.file == name);
                        self.app.st.desk.say(format!("Created {name}"));
                    }
                    Err(error) => self.app.st.desk.say(error),
                }
            }
            Action::DeskRefresh => {
                self.app.st.desk.menu = None;
                desk::refresh(&mut self.app.st.desk, true);
                self.app.st.desk.say("Desktop refreshed");
            }
            Action::QuickToggle => {
                self.app.st.quick_open = !self.app.st.quick_open;
                if !self.app.st.quick_open {
                    self.app.st.wifi.open = false;
                    self.app.st.bt.open = false;
                    self.app.st.wifi.pending = None;
                    if self.app.st.focus == Focus::WifiPassword {
                        self.app.st.focus = Focus::None;
                    }
                }
                if self.app.st.quick_open {
                    self.spawn_wifi_scan();
                }
            }
            Action::QuickClose => {
                self.app.st.quick_open = false;
                self.app.st.wifi.open = false;
                self.app.st.bt.open = false;
                self.app.st.wifi.pending = None;
                if self.app.st.focus == Focus::WifiPassword {
                    self.app.st.focus = Focus::None;
                }
            }
            Action::WifiList => {
                self.app.st.quick_open = true;
                self.app.st.wifi.open = !self.app.st.wifi.open;
                self.app.st.bt.open = false;
                if self.app.st.wifi.open {
                    self.spawn_wifi_scan();
                }
            }
            Action::WifiPower => {
                let on = !self.app.st.status.wifi_enabled;
                self.app.st.status.wifi_enabled = on;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    system::set_wifi_enabled(on);
                    std::thread::sleep(Duration::from_millis(400));
                    let _ = tx.send(Msg::Status(Box::new(ui::read_status())));
                });
            }
            Action::WifiRescan => self.spawn_wifi_scan(),
            Action::WifiPick(i) => {
                let Some(net) = self.app.st.wifi.networks.get(i).cloned() else {
                    return;
                };
                if net.active {
                    system::disconnect_wifi();
                    self.app.refresh_status();
                    return;
                }
                let sec = net.sec();
                if sec.needs_password() {
                    // A profile saved earlier may already hold the secret, so a
                    // known network joins without asking again.
                    if system::saved_wifi(&net.ssid).is_some() {
                        let ssid = net.ssid.clone();
                        let tx = self.tx.clone();
                        std::thread::spawn(move || {
                            let result = system::connect_saved_wifi(&ssid);
                            let _ = tx.send(Msg::ConnectSaved(ssid, result));
                        });
                        return;
                    }
                    self.app.st.wifi.pending = Some(net.ssid);
                    self.app.st.wifi.pending_security = sec;
                    self.app.st.wifi.password.clear();
                    self.app.st.wifi.error = None;
                    self.app.st.focus = Focus::WifiPassword;
                } else {
                    let ssid = net.ssid.clone();
                    let tx = self.tx.clone();
                    std::thread::spawn(move || {
                        let result = system::connect_wifi(&ssid, sec, None);
                        let _ = tx.send(Msg::Connect(result));
                    });
                }
            }
            Action::WifiPasswordChar(c) => {
                // Control characters would corrupt the passphrase silently.
                if c.is_control() {
                    return;
                }
                if self.app.st.wifi.password.chars().count() < 64 {
                    self.app.st.wifi.password.push(c);
                }
            }
            Action::WifiPasswordBackspace => {
                self.app.st.wifi.password.pop();
            }
            Action::WifiPasswordFocus => {
                if self.app.st.wifi.pending.is_some() {
                    self.app.st.focus = Focus::WifiPassword;
                }
            }
            Action::WifiPasswordSubmit => {
                let Some(ssid) = self.app.st.wifi.pending.clone() else {
                    return;
                };
                let password = self.app.st.wifi.password.clone();
                let sec = self.app.st.wifi.pending_security;
                self.app.st.wifi.error = None;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    let result = system::connect_wifi(&ssid, sec, Some(&password));
                    let _ = tx.send(Msg::Connect(result));
                });
            }
            Action::WifiForget => {}
            Action::BtPower => {
                let on = !self.app.st.status.bt.powered;
                self.app.st.status.bt.powered = on;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    system::set_bluetooth_powered(on);
                    std::thread::sleep(Duration::from_millis(400));
                    let _ = tx.send(Msg::Status(Box::new(ui::read_status())));
                });
            }
            Action::BtList => {
                self.app.st.quick_open = true;
                self.app.st.bt.open = !self.app.st.bt.open;
                self.app.st.wifi.open = false;
                if self.app.st.bt.open {
                    self.spawn_bt_refresh();
                }
            }
            Action::BtRescan => {
                self.app.st.bt.scanning = true;
                let tx = self.tx.clone();
                std::thread::spawn(move || {
                    system::bt_scan_blocking(6);
                    let devices = system::bt_devices();
                    let _ = tx.send(Msg::Bluetooth(devices));
                });
            }
            Action::BtPick(_) => {}
            Action::BtToggleDevice(i) => {
                if let Some(device) = self.app.st.bt.devices.get(i).cloned() {
                    if device.connected {
                        system::bt_disconnect(&device.mac);
                    } else {
                        system::bt_pair_and_connect(&device.mac);
                    }
                }
                self.spawn_bt_refresh();
            }
            Action::VolumeSet(fraction) => {
                if !fraction.is_finite() {
                    return;
                }
                // The sliders report a 0..1 position on their track; the track
                // covers the whole 0..160% range, so the last third of it is
                // amplification.
                let level = fraction.clamp(0.0, 1.0) * system::VOLUME_MAX;
                self.app.st.status.volume = level;
                self.app.st.status.muted = false;
                self.controls_touched = Some(Instant::now());
                let _ = self.volume_tx.send(level);
            }
            Action::BrightnessSet(level) => {
                if !level.is_finite() {
                    return;
                }
                let level = level.clamp(0.02, 1.0);
                self.app.st.status.brightness = Some(level);
                self.controls_touched = Some(Instant::now());
                let _ = self.brightness_tx.send(level);
            }
            Action::MuteToggle => {
                let muted = !self.app.st.status.muted;
                system::set_muted(muted);
                self.app.st.status.muted = muted;
                self.controls_touched = Some(Instant::now());
            }
            Action::WindowOpen(kind) => {
                self.app.st.window = Some(kind);
                self.app.st.quick_open = false;
                self.app.st.launcher_open = false;
                match kind {
                    WindowKind::Software => {
                        self.app.st.focus = Focus::Software;
                        self.spawn_package_search();
                    }
                    WindowKind::Settings => self.spawn_displays(),
                }
            }
            Action::WindowClose(kind) => {
                if self.app.st.window == Some(kind) {
                    self.app.st.window = None;
                }
                self.app.st.focus = Focus::None;
            }
            // The custom panel is the quick path; the full interface is the
            // program that already implements it. Launched as a child, so a
            // crash in it cannot affect the desktop.
            Action::OpenTool(tool) => {
                self.app.st.quick_open = false;
                self.app.st.launcher_open = false;
                let opened = match tool {
                    Tool::WifiSettings => system::open_wifi_settings(),
                    Tool::BluetoothDevices => system::open_bluetooth_settings(),
                    Tool::SoundSettings => system::open_sound_settings(),
                };
                if !opened {
                    eprintln!(
                        "refurb-shell: no program installed for {tool:?} \
                         (iwgtk, blueman-manager, pavucontrol)"
                    );
                }
            }
            Action::SoftwareChar(c) => {
                if self.app.st.software.query.chars().count() < 64 {
                    self.app.st.software.query.push(c);
                }
                self.search_due = Some(Instant::now() + Duration::from_millis(320));
            }
            Action::SoftwareBackspace => {
                self.app.st.software.query.pop();
                self.search_due = Some(Instant::now() + Duration::from_millis(320));
            }
            Action::SoftwareSuggest(label) => {
                self.app.st.software.query = pkg::category_term(&label)
                    .unwrap_or(&label)
                    .to_string();
                self.app.st.focus = Focus::Software;
                self.spawn_package_search();
            }
            Action::SoftwarePick(i) => {
                if i < self.app.st.software.results.len() {
                    self.app.st.software.selected = Some(i);
                }
            }
            Action::SoftwareAct => self.spawn_package_action(),
            Action::SoftwareScroll(delta) => {
                let count = self.app.st.software.results.len();
                let step = 3usize;
                let next = if delta > 0 {
                    (self.app.st.software.scroll + step).min(count.saturating_sub(1))
                } else {
                    self.app.st.software.scroll.saturating_sub(step)
                };
                self.app.st.software.scroll = next;
            }
            Action::Section(section) => {
                self.app.st.settings.section = section;
                self.app.st.window = Some(WindowKind::Settings);
                self.app.st.quick_open = false;
                if section == Section::Display {
                    self.spawn_displays();
                }
            }
            Action::Wallpaper(index) => {
                // The pixels are already decoded and cached, so the repaint at
                // the end of dispatch() is one blit per screen. Persisting the
                // choice is what makes it survive the next login.
                let choice = crate::wallpaper::Choice::from_index(index);
                self.app.st.wallpaper = choice;
                if let Err(e) = crate::wallpaper::activate(choice) {
                    eprintln!("refurb-shell: could not remember the wallpaper choice: {e}");
                }
            }
            Action::UpdateCheck => self.spawn_update(false),
            Action::UpdateInstall => self.spawn_update(true),
            Action::DisplayMode(i) => {
                let output = self
                    .app
                    .st
                    .display
                    .outputs
                    .get(self.app.st.display.selected)
                    .cloned();
                if let Some(output) = output {
                    if let Some(mode) = output.modes.get(i).cloned() {
                        if let Err(e) = system::set_display_mode(&output.name, &mode) {
                            eprintln!("refurb-shell: could not change mode: {e}");
                        }
                        self.spawn_displays();
                    }
                }
            }
            Action::Power(action) => {
                // Nothing happens to the power state until the dialog has been
                // answered. This is what stops the power key and the quick
                // panel's own buttons from shutting the machine down at once.
                self.app.st.confirm = Some(action);
                self.app.st.confirm_danger = false;
                self.app.st.quick_open = false;
                self.app.st.launcher_open = false;
                self.app.st.wifi.open = false;
                self.app.st.bt.open = false;
                self.app.st.wifi.pending = None;
                self.app.st.focus = Focus::None;
            }
            Action::DialogCancel => {
                self.app.st.confirm = None;
                self.app.st.confirm_danger = false;
            }
            Action::DialogFocus(danger) => self.app.st.confirm_danger = danger,
            Action::DialogConfirm => {
                let Some(action) = self.app.st.confirm else {
                    return;
                };
                self.app.st.confirm = None;
                self.app.st.confirm_danger = false;
                system::power(action);
            }
            Action::Lock => {
                self.app.st.quick_open = false;
                self.app.st.launcher_open = false;
                system::lock_screen();
            }
            Action::Quit => self.exit = true,
        }
    }

    // -- desktop ---------------------------------------------------------

    /// A taskbar icon focuses its running window, or starts the application.
    fn launch_pinned(&mut self, index: usize) {
        let Some(app) = self.app.st.apps.get(index).cloned() else {
            return;
        };
        if let Some(position) = self
            .app
            .st
            .toplevels
            .iter()
            .position(|toplevel| apps::matches_toplevel(&app, &toplevel.app_id))
        {
            if let (Some(entry), Some(seat)) = (self.toplevels.get(position), self.seat.as_ref()) {
                entry.handle.activate(seat);
            }
            return;
        }
        if let Err(error) = app.launch() {
            eprintln!("refurb-shell: could not launch {}: {error}", app.name);
        }
    }

    fn open_desktop_icon(&mut self) {
        let Some(index) = self.app.st.desk.selected else {
            return;
        };
        let Some(icon) = self.app.st.desk.icons.get(index).cloned() else {
            return;
        };
        self.app.st.desk.menu = None;
        if let Err(error) = desk::open(&icon) {
            self.app.st.desk.say(error);
        }
    }

    fn commit_rename(&mut self) {
        let Some((index, name)) = self.app.st.desk.renaming.take() else {
            return;
        };
        let Some(icon) = self.app.st.desk.icons.get(index).cloned() else {
            return;
        };
        match desk::rename(&icon, &name) {
            Ok(()) => {
                desk::refresh(&mut self.app.st.desk, true);
                self.app.st.desk.say(format!("Renamed to {name}"));
            }
            Err(error) => {
                self.app.st.desk.say(error);
                self.app.st.desk.renaming = Some((index, name));
            }
        }
    }

    /// A left press on an icon: select it, start a drag, or open it when this
    /// is the second press of a double click.
    fn desk_press(&mut self, index: usize) {
        if self.app.st.desk.renaming.is_some() {
            self.commit_rename();
            return;
        }
        let now = Instant::now();
        let double = self
            .app
            .st
            .desk
            .last_click
            .map(|(last, at)| last == index && now.duration_since(at) < Duration::from_millis(420))
            .unwrap_or(false);
        self.app.st.desk.last_click = Some((index, now));
        self.app.st.desk.selected = Some(index);
        self.app.st.desk.menu = None;
        if double {
            self.open_desktop_icon();
            return;
        }
        self.app.st.desk.press = Some(desk::Press {
            icon: index,
            at: self.pointer_pos,
        });
    }

    // -- background work -------------------------------------------------

    /// Read the machine on a worker thread. `nmcli` and `bluetoothctl` take
    /// hundreds of milliseconds each; doing that on the event loop froze the
    /// whole interface every 30 seconds.
    fn spawn_status_refresh(&mut self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let status = ui::read_status();
            let _ = tx.send(Msg::Status(Box::new(status)));
        });
    }

    fn spawn_wifi_scan(&mut self) {
        self.app.st.wifi.scanning = true;
        self.app.st.wifi.error = None;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let networks = system::scan_wifi();
            let _ = tx.send(Msg::Wifi(networks));
        });
    }

    fn spawn_bt_refresh(&mut self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let devices = system::bt_devices();
            let _ = tx.send(Msg::Bluetooth(devices));
        });
    }

    fn spawn_displays(&mut self) {
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let displays = system::displays();
            let _ = tx.send(Msg::Displays(displays));
        });
    }

    fn spawn_package_search(&mut self) {
        self.app.st.software.searched = true;
        let query = self.app.st.software.query.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let hits = pkg::search(&query, 80);
            let _ = tx.send(Msg::Packages(hits));
        });
    }

    fn spawn_package_action(&mut self) {
        let Some(index) = self.app.st.software.selected else {
            return;
        };
        let Some(package) = self.app.st.software.results.get(index).cloned() else {
            return;
        };
        if self.app.st.software.busy {
            return;
        }
        let install = !package.installed;
        self.app.st.software.busy = true;
        self.app.st.software.log.push(format!(
            "{} {} …",
            if install { "Installing" } else { "Removing" },
            package.name
        ));
        let tx = self.tx.clone();
        let name = package.name.clone();
        std::thread::spawn(move || {
            let (line_tx, line_rx) = mpsc::channel();
            pkg::run_action(install, &name, line_tx);
            for event in line_rx {
                match event {
                    pkg::Event::Line(line) => {
                        let _ = tx.send(Msg::Log(line));
                    }
                    pkg::Event::Done(ok) => {
                        let _ = tx.send(Msg::PackageDone(ok, name.clone()));
                    }
                }
            }
        });
    }

    fn spawn_update(&mut self, install: bool) {
        self.app.st.updates_note.clear();
        self.app
            .st
            .updates_note
            .push(if install {
                "Installing updates…".to_string()
            } else {
                "Checking for updates…".to_string()
            });
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            if install {
                use std::io::{BufRead, BufReader};
                let child = std::process::Command::new("sudo")
                    .args(["-n", "/usr/lib/refurb/refurb-update", "--install"])
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn();
                match child {
                    Ok(mut child) => {
                        if let Some(out) = child.stdout.take() {
                            for line in BufReader::new(out).lines().map_while(Result::ok) {
                                let _ = tx.send(Msg::Log(line));
                            }
                        }
                        let ok = child.wait().map(|s| s.success()).unwrap_or(false);
                        let _ = tx.send(Msg::Updates(if ok {
                            Ok((0, 0))
                        } else {
                            Err("Update installation failed.".into())
                        }));
                    }
                    Err(e) => {
                        let _ = tx.send(Msg::Updates(Err(format!("Could not start updater: {e}"))));
                    }
                }
            } else {
                let result = system::run_update_check();
                let _ = tx.send(Msg::Updates(result));
            }
        });
    }

    fn on_msg(&mut self, msg: Msg) {
        match msg {
            Msg::Wifi(networks) => {
                self.app.st.wifi.networks = networks;
                self.app.st.wifi.scanning = false;
            }
            Msg::Connect(result) => match result {
                Ok(()) => {
                    self.app.st.wifi.pending = None;
                    self.app.st.wifi.password.clear();
                    self.app.st.wifi.error = None;
                    self.app.st.wifi.open = false;
                    self.app.st.focus = Focus::None;
                    self.spawn_status_refresh();
                }
                Err(error) => {
                    // Keep the dialog open, keep the focus and keep what was
                    // typed: a typo must be correctable, not a dead end.
                    self.app.st.wifi.error = Some(error);
                    self.app.st.focus = Focus::WifiPassword;
                }
            },
            Msg::ConnectSaved(ssid, result) => match result {
                Ok(()) => {
                    self.app.st.wifi.pending = None;
                    self.app.st.wifi.error = None;
                    self.app.st.wifi.open = false;
                    self.app.st.focus = Focus::None;
                    self.spawn_status_refresh();
                }
                Err(error) if system::wants_password(&error) => {
                    // The saved profile exists but has no usable secret: ask for
                    // one instead of showing a NetworkManager internal error.
                    let sec = self
                        .app
                        .st
                        .wifi
                        .networks
                        .iter()
                        .find(|net| net.ssid == ssid)
                        .map(|net| net.sec())
                        .unwrap_or(system::WifiSec::WpaPsk);
                    self.app.st.wifi.pending = Some(ssid);
                    self.app.st.wifi.pending_security = sec;
                    self.app.st.wifi.password.clear();
                    self.app.st.wifi.error = None;
                    self.app.st.focus = Focus::WifiPassword;
                }
                Err(error) => {
                    self.app.st.wifi.error = Some(error);
                    self.app.st.focus = Focus::None;
                }
            },
            Msg::Bluetooth(devices) => {
                self.app.st.bt.devices = devices;
                self.app.st.bt.scanning = false;
            }
            Msg::Packages(hits) => {
                self.app.st.software.selected = if hits.is_empty() { None } else { Some(0) };
                self.app.st.software.results = hits;
                self.app.st.software.scroll = 0;
            }
            Msg::Log(line) => {
                if self.app.st.software.log.len() > 200 {
                    self.app.st.software.log.remove(0);
                }
                self.app.st.software.log.push(line.clone());
                if self.app.st.updates_note.len() < 60 {
                    self.app.st.updates_note.push(line);
                }
            }
            Msg::PackageDone(ok, name) => {
                self.app.st.software.busy = false;
                self.app.st.software.log.push(if ok {
                    format!("Finished {name}.")
                } else {
                    format!("Could not change {name}.")
                });
                self.spawn_package_search();
            }
            Msg::Updates(result) => match result {
                Ok((upgrades, security)) => {
                    self.app.st.status.updates = security;
                    self.app.st.updates_note.push(format!(
                        "{upgrades} package updates available ({security} security)."
                    ));
                    self.spawn_status_refresh();
                }
                Err(error) => self.app.st.updates_note.push(error),
            },
            Msg::Displays(displays) => {
                self.app.st.display.outputs = displays;
                self.app.st.display.selected = 0;
            }
            Msg::Status(status) => {
                // A poll that lands while a slider is being dragged must not
                // pull the control back to a level the user has already left.
                let keep = self.drag.is_some()
                    || self
                        .controls_touched
                        .map(|when| when.elapsed() < Duration::from_millis(1500))
                        .unwrap_or(false);
                self.app.apply_status(*status, keep);
            }
            Msg::VolumeApplied(result) => match result {
                Ok((level, muted)) => {
                    // The value just read back is the truth; only a drag in
                    // progress is allowed to keep the pointer's position.
                    if self.drag.is_none() {
                        self.app.st.status.volume = level;
                        self.app.st.status.muted = muted;
                    }
                }
                Err(error) => self.note(error),
            },
            Msg::BrightnessApplied(result) => match result {
                Ok(actual) => {
                    // Show what the hardware did, not what was requested.
                    if self.drag.is_none() {
                        self.app.st.status.brightness = Some(actual);
                        self.app.st.status.backlight = true;
                    }
                }
                Err(error) => {
                    // Never leave a control that lies about the panel.
                    eprintln!("refurb-shell: brightness: {error}");
                    self.app.st.status.backlight = false;
                }
            },
        }
        self.sync_panels();
        self.redraw_visible();
    }

    // -- input -----------------------------------------------------------

    fn on_key(&mut self, key: Key) {
        // A desktop icon being renamed owns the keyboard until Enter or Escape.
        if self.app.st.desk.renaming.is_some() {
            match key {
                Key::Char(c) => self.dispatch(Action::DeskRenameChar(c)),
                Key::Backspace => self.dispatch(Action::DeskRenameBackspace),
                Key::Enter => self.dispatch(Action::DeskRenameCommit),
                Key::Escape => self.dispatch(Action::DeskRenameCancel),
                _ => {}
            }
            return;
        }
        // The confirmation is modal: while it is open, Escape cancels and the
        // arrow keys or Tab move between the two buttons. Focus starts on
        // Cancel, so Enter without a deliberate move never powers off.
        if self.app.st.confirm.is_some() {
            match key {
                Key::Escape => self.dispatch(Action::DialogCancel),
                Key::Tab | Key::Left | Key::Right | Key::Up | Key::Down => {
                    let next = !self.app.st.confirm_danger;
                    self.dispatch(Action::DialogFocus(next));
                }
                Key::Enter => {
                    if self.app.st.confirm_danger {
                        self.dispatch(Action::DialogConfirm);
                    } else {
                        self.dispatch(Action::DialogCancel);
                    }
                }
                _ => {}
            }
            return;
        }
        if self.app.st.quick_open && self.app.st.wifi.pending.is_some() {
            match key {
                Key::Char(c) => self.dispatch(Action::WifiPasswordChar(c)),
                Key::Backspace => self.dispatch(Action::WifiPasswordBackspace),
                Key::Enter => self.dispatch(Action::WifiPasswordSubmit),
                Key::Escape => {
                    self.app.st.wifi.pending = None;
                    self.app.st.wifi.password.clear();
                    self.app.st.focus = Focus::None;
                    self.sync_panels();
                    self.redraw(Surface::Quick);
                }
                _ => {}
            }
            return;
        }
        if self.app.st.launcher_open {
            match key {
                Key::Char(c) => self.dispatch(Action::LauncherChar(c)),
                Key::Backspace => self.dispatch(Action::LauncherBackspace),
                Key::Enter => {
                    let first = self.app.launcher_results().first().copied();
                    match first {
                        Some(index) => self.dispatch(Action::Launch(index)),
                        None => self.dispatch(Action::SearchWeb),
                    }
                }
                Key::Escape => self.dispatch(Action::LauncherClose),
                _ => {}
            }
            return;
        }
        if self.app.st.quick_open {
            if key == Key::Escape {
                self.dispatch(Action::QuickClose);
            }
            return;
        }
        match self.app.st.window {
            Some(WindowKind::Software) => match key {
                Key::Char(c) => self.dispatch(Action::SoftwareChar(c)),
                Key::Backspace => self.dispatch(Action::SoftwareBackspace),
                Key::Down => self.move_package_selection(1),
                Key::Up => self.move_package_selection(-1),
                Key::Enter => self.dispatch(Action::SoftwareAct),
                Key::Escape => self.dispatch(Action::WindowClose(WindowKind::Software)),
                _ => {}
            },
            Some(WindowKind::Settings) => {
                if key == Key::Escape {
                    self.dispatch(Action::WindowClose(WindowKind::Settings));
                }
            }
            None => {}
        }
    }

    fn move_package_selection(&mut self, delta: i32) {
        let count = self.app.st.software.results.len();
        if count == 0 {
            return;
        }
        let current = self.app.st.software.selected.unwrap_or(0) as i32;
        let next = (current + delta).clamp(0, count as i32 - 1) as usize;
        self.app.st.software.selected = Some(next);
        self.redraw_visible();
    }

    fn on_pointer_button(&mut self, pressed: bool, button: u32, serial: u32) {
        const LEFT: u32 = 0x110;
        const RIGHT: u32 = 0x111;
        if !pressed {
            self.pointer_down = false;
            if self.app.st.desk.press.take().is_some() {
                // Dropping an icon is the moment its position becomes the one
                // the next session opens with.
                desk::save_layout(&self.app.st.desk.icons);
            }
            self.drag = None;
            return;
        }
        let Some(surface) = self.pointer_focus else {
            return;
        };
        let Some(idx) = self.entry_index(surface) else {
            return;
        };
        let hits = self.surfaces[idx].hits.clone();
        let (x, y) = (self.pointer_pos.0 as f32, self.pointer_pos.1 as f32);
        let found = hits
            .iter()
            .rev()
            .find(|(rect, _)| crate::render::contains(*rect, x, y))
            .map(|(rect, action)| (*rect, action.clone()));

        // A right click opens the icon menu, or the desktop's own menu on the
        // empty wallpaper. There is no other right-click behaviour in the shell.
        if button == RIGHT {
            self.pointer_down = false;
            if !matches!(surface, Surface::Desktop) {
                return;
            }
            match found {
                Some((_, Action::DeskIcon(index))) => self.dispatch(Action::DeskMenu(index)),
                _ => self.dispatch(Action::DeskMenuEmpty),
            }
            return;
        }
        if button != LEFT {
            return;
        }
        self.pointer_down = true;

        let Some((rect, action)) = found else {
            // A press on the empty part of a shell surface is "outside" as far
            // as the quick panel is concerned.
            if self.app.st.desk.menu.is_some() {
                self.dispatch(Action::DeskMenuClose);
            }
            if self.app.st.quick_open {
                self.dispatch(Action::QuickClose);
            }
            self.note(format!("press at {x},{y} hit nothing on {surface:?}"));
            return;
        };
        // Clicking the desktop, the bar or a window dismisses the quick panel.
        // The three controls that open it are exempt, because those toggle it.
        if self.app.st.quick_open
            && !matches!(surface, Surface::Quick)
            && !matches!(
                action,
                Action::QuickToggle | Action::WifiList | Action::BtList
            )
        {
            self.dispatch(Action::QuickClose);
        }
        self.note(format!("press at {x},{y} on {surface:?} -> {action:?} rect {rect:?}"));
        // Sliders carry their own track rect, so the value is taken from the
        // click position and the drag continues from there.
        match action {
            Action::VolumeSet(_) => {
                if let Some(value) = fraction(rect, x) {
                    self.drag = Some((SliderKind::Volume, rect));
                    self.dispatch(Action::VolumeSet(value));
                }
            }
            Action::BrightnessSet(_) => {
                if let Some(value) = fraction(rect, x) {
                    self.drag = Some((SliderKind::Brightness, rect));
                    self.dispatch(Action::BrightnessSet(value));
                }
            }
            other => {
                let _ = serial;
                self.dispatch(other);
                // Apply the effect of the click before the next input event in
                // this batch is handled. Expanding a section changes the panel's
                // height, and a click that arrives in the same batch must be
                // matched against the expanded layout instead of the old one.
                self.sync_panels();
            }
        }
    }

    fn on_pointer_motion(&mut self, position: (f64, f64)) {
        self.pointer_pos = position;
        // Dragging a desktop icon moves it to the nearest free cell; the icon
        // never leaves the visible grid, so it cannot be lost off-screen.
        if self.pointer_down {
            if let Some(press) = self.app.st.desk.press.clone() {
                let (mut col, mut row) = desk::cell_at(
                    position.0 as f32 - desk::CELL_W / 2.0,
                    position.1 as f32 - desk::CELL_H / 2.0,
                );
                row = row.clamp(0, desk::ROWS - 1);
                col = col.max(0);
                let current = self
                    .app
                    .st
                    .desk
                    .icons
                    .get(press.icon)
                    .map(|icon| (icon.col, icon.row));
                if current != Some((col, row)) {
                    if let Some((c, r)) =
                        desk::free_cell(&self.app.st.desk.icons, col, row, press.icon)
                    {
                        if let Some(icon) = self.app.st.desk.icons.get_mut(press.icon) {
                            icon.col = c;
                            icon.row = r;
                        }
                        self.invalidate(Some(Surface::Desktop));
                    }
                }
                return;
            }
        }
        if let Some((kind, rect)) = self.drag {
            if let Some(value) = fraction(rect, position.0 as f32) {
                match kind {
                    SliderKind::Volume => self.dispatch(Action::VolumeSet(value)),
                    SliderKind::Brightness => self.dispatch(Action::BrightnessSet(value)),
                }
            }
            return;
        }
        self.app.st.hover = Some((position.0 as f32, position.1 as f32));
        if let Some(surface) = self.pointer_focus {
            // The wallpaper has nothing to hover. Repainting a full-screen
            // surface for every motion event is pure CPU burn on the machines
            // this image targets, and it made open applications feel slow.
            if !matches!(surface, Surface::Desktop | Surface::DesktopOn(_)) {
                self.invalidate(Some(surface));
            }
        }
    }

    fn sync_toplevels(&mut self) {
        self.toplevels.retain(|entry| !entry.closed);
        self.app.st.toplevels = self
            .toplevels
            .iter()
            .enumerate()
            .map(|(index, entry)| ui::Toplevel {
                handle: index,
                title: if entry.title.is_empty() {
                    entry.app_id.clone()
                } else {
                    entry.title.clone()
                },
                app_id: entry.app_id.clone(),
                active: entry.active,
            })
            .collect();
        self.redraw_bars();
    }

    // -- event loop ------------------------------------------------------

    fn poll_ipc(&mut self) {
        let Some(listener) = &self.ipc else {
            return;
        };
        let Ok((mut stream, _)) = listener.accept() else {
            return;
        };
        let mut buffer = [0u8; 64];
        let n = stream.read(&mut buffer).unwrap_or(0);
        if n == 0 {
            return;
        }
        let command = String::from_utf8_lossy(&buffer[..n]).trim().to_string();
        // The confirmation is modal for the compositor's shortcuts too: while it
        // is open, Super+Space and friends must not open a panel on top of it.
        if self.app.st.confirm.is_some() && !matches!(command.as_str(), "power" | "quit") {
            return;
        }
        match command.as_str() {
            "launcher" => {
                if self.app.st.launcher_open {
                    self.dispatch(Action::LauncherClose);
                } else {
                    self.dispatch(Action::LauncherOpen);
                }
            }
            "quick" => self.dispatch(Action::QuickToggle),
            "settings" => self.dispatch(Action::WindowOpen(WindowKind::Settings)),
            "software" => self.dispatch(Action::WindowOpen(WindowKind::Software)),
            "lock" => self.dispatch(Action::Lock),
            // The machine's own power key (labwc keybind): the dialog answers
            // this, not logind, so a short press can never lose open work.
            "power" => self.dispatch(Action::Power(system::PowerAction::ShutDown)),
            "quit" => self.dispatch(Action::Quit),
            other => eprintln!("refurb-shell: unknown command {other:?}"),
        }
    }

    fn pump(&mut self) {
        self.poll_ipc();
        while let Ok(msg) = self.rx.try_recv() {
            self.on_msg(msg);
        }
        let now = Instant::now();
        if now.duration_since(self.last_tick) >= Duration::from_secs(1) {
            self.last_tick = now;
            let previous = self.app.st.clock.clone();
            self.app.tick();
            if self.app.st.clock != previous {
                self.redraw_bars();
            }
            // Monitors can appear or disappear at any time. Registry events are
            // the primary signal, but on the r8 test image a second GPU's output
            // never produced one, so the list is also reconciled against the
            // registry snapshot once a second. Comparing a handful of globals
            // costs nothing when nothing changed.
            self.reconcile_outputs();
        }
        // `refresh` compares a cheap directory signature and rescans only when
        // a file was added, changed or removed.
        if desk::refresh(&mut self.app.st.desk, false) {
            self.invalidate(Some(Surface::Desktop));
        }
        if self.app.st.desk.expire_notice() {
            self.invalidate(Some(Surface::Desktop));
        }
        if now.duration_since(self.last_dictation) >= Duration::from_millis(900) {
            self.last_dictation = now;
            let state = dictation_state();
            if state != self.dictation {
                self.dictation = state.clone();
                self.app.st.dictation = state;
                self.redraw_bars();
            }
        }
        if now.duration_since(self.last_status) >= Duration::from_secs(30) {
            self.last_status = now;
            self.spawn_status_refresh();
        }
        if let Some(due) = self.search_due {
            if now >= due {
                self.search_due = None;
                if self.app.st.window == Some(WindowKind::Software) {
                    self.spawn_package_search();
                }
            }
        }
    }

    fn poll_timeout(&self) -> c_int {
        let now = Instant::now();
        let mut wait = Duration::from_secs(1);
        if let Some(due) = self.search_due {
            let until = due.saturating_duration_since(now);
            wait = wait.min(until);
        }
        if !self.dirty.is_empty() {
            // A frame is owed: wake up when the frame interval has elapsed.
            let due = self.last_paint + Duration::from_millis(14);
            wait = wait.min(due.saturating_duration_since(now));
        }
        wait.as_millis().min(1000) as c_int
    }

    fn loop_forever(&mut self, queue: &mut EventQueue<Session>) -> Result<(), String> {
        let raw = self.conn_fd();
        loop {
            self.pump();
            self.flush_dirty();
            if self.exit {
                break;
            }
            queue
                .flush()
                .map_err(|e| format!("lost connection to the compositor: {e}"))?;
            if let Some(guard) = queue.prepare_read() {
                let ipc_fd = self
                    .ipc
                    .as_ref()
                    .map(|listener| listener.as_raw_fd())
                    .unwrap_or(-1);
                let mut fds = [
                    PollFd {
                        fd: raw,
                        events: POLLIN,
                        revents: 0,
                    },
                    PollFd {
                        fd: ipc_fd,
                        events: POLLIN,
                        revents: 0,
                    },
                ];
                let timeout = self.poll_timeout();
                let ready = unsafe { poll(fds.as_mut_ptr(), 2, timeout) };
                if ready > 0 && fds[0].revents & POLLIN != 0 {
                    let _ = guard.read();
                }
            }
            queue
                .dispatch_pending(self)
                .map_err(|e| format!("dispatch: {e}"))?;
        }
        Ok(())
    }

    fn conn_fd(&self) -> c_int {
        // The Wayland socket; kept as a raw descriptor for poll(2).
        self.conn.backend().poll_fd().as_raw_fd()
    }
}

/// Record a panic where the session log can see it. The supervisor already
/// captures stderr; without this a crash inside the shell only looked like
/// "the desktop disappeared and came back".
fn install_panic_log() {
    std::panic::set_hook(Box::new(|info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        eprintln!("refurb-shell: panic: {info}\n{backtrace}");
    }));
}

/// Hardware controls run on their own threads, one value at a time.
///
/// A slider drag produces a position for every pointer motion event. Applying
/// each one launched a process, so the writes raced, arrived out of order and
/// the sink often ended on an older value than the one under the pointer. The
/// workers below keep only the newest requested value, so the UI stays
/// instant and the hardware ends exactly where the user let go.
fn start_control_workers(tx: &Sender<Msg>) -> (Sender<f32>, Sender<f32>) {
    let (volume_tx, volume_rx) = mpsc::channel::<f32>();
    let volume_msgs = tx.clone();
    std::thread::spawn(move || {
        while let Ok(mut level) = volume_rx.recv() {
            while let Ok(newer) = volume_rx.try_recv() {
                level = newer;
            }
            let ok = std::panic::catch_unwind(|| system::set_volume(level)).unwrap_or(false);
            let applied = if ok {
                Ok(system::volume())
            } else {
                Err("the audio sink refused the new volume".to_string())
            };
            let _ = volume_msgs.send(Msg::VolumeApplied(applied));
        }
    });

    let (brightness_tx, brightness_rx) = mpsc::channel::<f32>();
    let brightness_msgs = tx.clone();
    std::thread::spawn(move || {
        while let Ok(mut level) = brightness_rx.recv() {
            while let Ok(newer) = brightness_rx.try_recv() {
                level = newer;
            }
            let result = std::panic::catch_unwind(|| system::set_brightness(level))
                .unwrap_or_else(|_| Err("the brightness change failed".to_string()));
            let _ = brightness_msgs.send(Msg::BrightnessApplied(result));
        }
    });

    (volume_tx, brightness_tx)
}

fn surface_for(kind: WindowKind) -> Surface {
    match kind {
        WindowKind::Software => Surface::Software,
        WindowKind::Settings => Surface::Settings,
    }
}

/// What the Alt+Z dictation helper says it is doing. The helper writes one
/// line and removes the file when it exits; an error is shown for six seconds
/// so a missing microphone is visible without becoming a permanent badge.
fn dictation_state() -> Option<String> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let path = dir.join("refurb-dictate.state");
    let meta = std::fs::metadata(&path).ok()?;
    let text = std::fs::read_to_string(&path).ok()?.trim().to_string();
    if text.is_empty() {
        return None;
    }
    if text.starts_with("error") {
        let stale = meta
            .modified()
            .ok()
            .and_then(|when| when.elapsed().ok())
            .map(|age| age.as_secs() > 6)
            .unwrap_or(false);
        if stale {
            return None;
        }
    }
    Some(text)
}

fn fraction(rect: Rect, x: f32) -> Option<f32> {
    if rect.width() <= 1.0 {
        return None;
    }
    Some(((x - rect.x()) / rect.width()).clamp(0.0, 1.0))
}

fn url_encode(text: &str) -> String {
    let mut out = String::new();
    for byte in text.as_bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            b' ' => out.push('+'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// IPC: a single socket in XDG_RUNTIME_DIR lets the compositor's keybindings
// (and `refurb-shell --send`) drive the same actions as the interface.
// ---------------------------------------------------------------------------

fn ipc_path() -> PathBuf {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    dir.join("refurb-shell.sock")
}

fn start_ipc() -> Option<UnixListener> {
    let path = ipc_path();
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(&path).ok()?;
    listener.set_nonblocking(true).ok()?;
    // Only the session user may talk to the shell.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Some(listener)
}

/// Send a command to a running shell. Used by labwc keybindings.
pub fn send_command(command: &str) -> Result<(), String> {
    let path = ipc_path();
    let mut stream = UnixStream::connect(&path)
        .map_err(|e| format!("cannot reach the running shell ({e})"))?;
    stream
        .write_all(command.as_bytes())
        .map_err(|e| format!("cannot send command: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Dispatch implementations
// ---------------------------------------------------------------------------

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Session {
    fn event(
        session: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        // Monitors can be plugged in or unplugged while the desktop is running.
        // The shell used to bind every screen once at startup and ignore these
        // events, so a second monitor attached later stayed black and the bar
        // never appeared on it. Both directions rebuild the per-screen surfaces
        // from the registry's current output list.
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } => {
                if interface != "wl_output" {
                    return;
                }
                if session.output_globals.iter().any(|(n, _)| *n == name) {
                    return;
                }
                let output =
                    registry.bind::<wl_output::WlOutput, _, _>(name, version.min(4), qh, ());
                session.output_globals.push((name, output.clone()));
                session.outputs.push(output);
                session.note(format!(
                    "screen connected (global {name}); {} screen(s)",
                    session.outputs.len()
                ));
                session.rebuild_screens();
            }
            wl_registry::Event::GlobalRemove { name } => {
                let Some(pos) = session.output_globals.iter().position(|(n, _)| *n == name) else {
                    return;
                };
                session.output_globals.remove(pos);
                if pos < session.outputs.len() {
                    session.outputs.remove(pos);
                }
                session.note(format!(
                    "screen disconnected (global {name}); {} screen(s)",
                    session.outputs.len()
                ));
                session.rebuild_screens();
            }
            _ => {}
        }
    }
}

delegate_noop!(Session: ignore wl_compositor::WlCompositor);
delegate_noop!(Session: ignore wl_shm::WlShm);
delegate_noop!(Session: ignore wl_shm_pool::WlShmPool);
delegate_noop!(Session: ignore wl_output::WlOutput);
delegate_noop!(Session: ignore zwlr_layer_shell_v1::ZwlrLayerShellV1);

impl Dispatch<wl_buffer::WlBuffer, Arc<AtomicBool>> for Session {
    fn event(
        _: &mut Self,
        _: &wl_buffer::WlBuffer,
        event: wl_buffer::Event,
        data: &Arc<AtomicBool>,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_buffer::Event::Release = event {
            data.store(false, Ordering::Relaxed);
        }
    }
}

impl Dispatch<wl_surface::WlSurface, Surface> for Session {
    fn event(
        _: &mut Self,
        _: &wl_surface::WlSurface,
        _: wl_surface::Event,
        _: &Surface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<xdg_wm_base::XdgWmBase, ()> for Session {
    fn event(
        _: &mut Self,
        proxy: &xdg_wm_base::XdgWmBase,
        event: xdg_wm_base::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_wm_base::Event::Ping { serial } = event {
            proxy.pong(serial);
        }
    }
}

impl Dispatch<xdg_surface::XdgSurface, Surface> for Session {
    fn event(
        session: &mut Self,
        proxy: &xdg_surface::XdgSurface,
        event: xdg_surface::Event,
        id: &Surface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            proxy.ack_configure(serial);
            if let Some(index) = session.entry_index(*id) {
                session.surfaces[index].configured = true;
            }
            session.redraw(*id);
        }
    }
}

impl Dispatch<xdg_toplevel::XdgToplevel, Surface> for Session {
    fn event(
        session: &mut Self,
        _: &xdg_toplevel::XdgToplevel,
        event: xdg_toplevel::Event,
        id: &Surface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            xdg_toplevel::Event::Configure { width, height, .. } => {
                if let Some(index) = session.entry_index(*id) {
                    if width > 0 && height > 0 {
                        session.surfaces[index].width = width as u32;
                        session.surfaces[index].height = height as u32;
                    }
                }
            }
            xdg_toplevel::Event::Close => {
                session.app.st.window = None;
                session.sync_panels();
                session.redraw_visible();
            }
            _ => {}
        }
    }
}

impl Dispatch<zwlr_layer_surface_v1::ZwlrLayerSurfaceV1, Surface> for Session {
    fn event(
        session: &mut Self,
        proxy: &zwlr_layer_surface_v1::ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        id: &Surface,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                proxy.ack_configure(serial);
                if let Some(index) = session.entry_index(*id) {
                    if width > 0 {
                        session.surfaces[index].width = width;
                    }
                    if height > 0 {
                        session.surfaces[index].height = height;
                    }
                    session.surfaces[index].configured = true;
                }
                session.redraw(*id);
            }
            zwlr_layer_surface_v1::Event::Closed => {
                session.close_surface(*id);
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_seat::WlSeat, ()> for Session {
    fn event(
        session: &mut Self,
        seat: &wl_seat::WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities { capabilities } = event {
            let WEnum::Value(capabilities) = capabilities else {
                return;
            };
            // Create input objects only for advertised capabilities, and
            // release them when a capability disappears (an undocked laptop
            // loses its keyboard; a touch-only device never has one). Creating
            // them eagerly is a protocol error, so is keeping them alive after
            // the capability is gone.
            if capabilities.contains(wl_seat::Capability::Keyboard) {
                if session.keyboard.is_none() {
                    session.keyboard = Some(seat.get_keyboard(qh, ()));
                }
            } else if let Some(keyboard) = session.keyboard.take() {
                if keyboard.version() >= 3 {
                    keyboard.release();
                }
            }
            if capabilities.contains(wl_seat::Capability::Pointer) {
                if session.pointer.is_none() {
                    session.pointer = Some(seat.get_pointer(qh, ()));
                }
            } else if let Some(pointer) = session.pointer.take() {
                if pointer.version() >= 3 {
                    pointer.release();
                }
            }
        }
    }
}

impl Dispatch<wl_keyboard::WlKeyboard, ()> for Session {
    fn event(
        session: &mut Self,
        _: &wl_keyboard::WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if matches!(format, WEnum::Value(wl_keyboard::KeymapFormat::XkbV1)) {
                    use std::io::Read;
                    let mut file = std::fs::File::from(fd);
                    // Read the descriptor to its end instead of trusting the
                    // announced size. A short read used to leave an empty
                    // string, libxkbcommon then failed to parse it and every
                    // machine silently got the built-in US layout - which is
                    // invisible on a US image and wrong on any other layout.
                    let mut raw = Vec::new();
                    let _ = file.read_to_end(&mut raw);
                    if size > 0 && raw.len() > size as usize {
                        raw.truncate(size as usize);
                    }
                    if !raw.is_empty() {
                        let text = String::from_utf8_lossy(&raw);
                        let text = match text.find('\0') {
                            Some(end) => &text[..end],
                            None => &text[..],
                        };
                        session.keymap = Keymap::from_keymap_string(text);
                    }
                    if !session.keymap.uses_xkb() {
                        eprintln!(
                            "refurb-shell: could not load the compositor keymap ({} bytes, {} announced); using the built-in US layout",
                            raw.len(),
                            size
                        );
                    }
                }
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                session.modifiers = Modifiers {
                    depressed: mods_depressed,
                    latched: mods_latched,
                    locked: mods_locked,
                    group,
                };
                session.keymap.update_modifiers(&session.modifiers);
            }
            wl_keyboard::Event::Key { key, state, .. } => {
                if matches!(state, WEnum::Value(wl_keyboard::KeyState::Pressed)) {
                    let shift = session.modifiers.shift();
                    if let Some(translated) = session.keymap.translate(key, shift) {
                        session.on_key(translated);
                    }
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<wl_pointer::WlPointer, ()> for Session {
    fn event(
        session: &mut Self,
        _: &wl_pointer::WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                session.pointer_focus = surface.data::<Surface>().copied();
                session.on_pointer_motion((surface_x, surface_y));
            }
            wl_pointer::Event::Leave { .. } => {
                session.pointer_focus = None;
                session.app.st.hover = None;
                // A drag that ends with the pointer outside the surface used to
                // leave the shell thinking it was still dragging, and the next
                // poll then fought the slider.
                session.drag = None;
                session.redraw_visible();
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => session.on_pointer_motion((surface_x, surface_y)),
            wl_pointer::Event::Button {
                serial,
                button,
                state,
                ..
            } if button == 0x110 || button == 0x111 => {
                // BTN_LEFT and BTN_RIGHT. The right button is only used for the
                // desktop's context menu.
                let pressed = matches!(state, WEnum::Value(wl_pointer::ButtonState::Pressed));
                session.on_pointer_button(pressed, button, serial);
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                if matches!(axis, WEnum::Value(wl_pointer::Axis::VerticalScroll)) && value != 0.0 {
                    session.dispatch(Action::SoftwareScroll(if value > 0.0 { 1 } else { -1 }));
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, ()> for Session {
    fn event(
        session: &mut Self,
        _: &zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1,
        event: zwlr_foreign_toplevel_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let zwlr_foreign_toplevel_manager_v1::Event::Toplevel { toplevel } = event {
            session.toplevels.push(TlEntry {
                handle: toplevel,
                title: String::new(),
                app_id: String::new(),
                active: false,
                closed: false,
            });
        }
    }

    wayland_client::event_created_child!(Session, zwlr_foreign_toplevel_manager_v1::ZwlrForeignToplevelManagerV1, [
        zwlr_foreign_toplevel_manager_v1::EVT_TOPLEVEL_OPCODE => (zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ())
    ]);
}

impl Dispatch<zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1, ()> for Session {
    fn event(
        session: &mut Self,
        proxy: &zwlr_foreign_toplevel_handle_v1::ZwlrForeignToplevelHandleV1,
        event: zwlr_foreign_toplevel_handle_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let id = proxy.id().protocol_id();
        let mut sync = false;
        {
            let Some(entry) = session
                .toplevels
                .iter_mut()
                .find(|entry| entry.handle.id().protocol_id() == id)
            else {
                return;
            };
            match event {
                zwlr_foreign_toplevel_handle_v1::Event::Title { title } => entry.title = title,
                zwlr_foreign_toplevel_handle_v1::Event::AppId { app_id } => entry.app_id = app_id,
                zwlr_foreign_toplevel_handle_v1::Event::State { state } => {
                    entry.active = state.contains(&2u8);
                }
                zwlr_foreign_toplevel_handle_v1::Event::Closed => {
                    entry.closed = true;
                    // A closed window is removed from the taskbar immediately.
                    // Waiting for a later `Done` left the entry of a destroyed
                    // window in the bar until something else happened.
                    sync = true;
                }
                zwlr_foreign_toplevel_handle_v1::Event::Done => sync = true,
                _ => {}
            }
        }
        if sync {
            session.sync_toplevels();
        }
    }
}
