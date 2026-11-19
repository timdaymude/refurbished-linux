//! Headless rendering of the whole interface.
//!
//! Used for screenshots, for reviewing the design without a compositor and as
//! the visual part of the test suite: `refurb-shell --render-preview out/`
//! writes a PNG per screen from the same drawing code the session uses.

use std::path::{Path, PathBuf};

use tiny_skia::{BlendMode, FilterQuality, Pixmap, PixmapPaint, Transform};

use crate::apps::App as DesktopApp;
use crate::system::{BtDevice, BtStatus, WifiNetwork, WifiStatus};
use crate::text::{TextEngine, REGULAR};
use crate::ui::{self, App, DisplayOutput, Focus, Pkg, Surface, Toplevel, WindowKind};
use crate::{icons, render, theme};

fn paint(base: &mut Pixmap, top: &Pixmap, x: i32, y: i32) {
    base.draw_pixmap(
        x,
        y,
        top.as_ref(),
        &PixmapPaint {
            opacity: 1.0,
            blend_mode: BlendMode::SourceOver,
            quality: FilterQuality::Nearest,
        },
        Transform::identity(),
        None,
    );
}

fn save(pm: &Pixmap, path: PathBuf) -> std::io::Result<PathBuf> {
    let png = pm
        .encode_png()
        .map_err(|e| std::io::Error::other(format!("encode {path:?}: {e}")))?;
    std::fs::write(&path, png)?;
    Ok(path)
}

/// Representative state so the screenshots show a machine in real use.
pub fn seed(app: &mut App) {
    let mk = |id: &str, name: &str, comment: &str, exec: &str, pin: u8| DesktopApp {
        id: id.into(),
        name: name.into(),
        comment: comment.into(),
        exec: exec.into(),
        pin: Some(pin),
        ..Default::default()
    };
    let web = |id: &str, name: &str, comment: &str, url: &str, pin: u8| DesktopApp {
        id: id.into(),
        name: name.into(),
        comment: comment.into(),
        exec: format!("brave-browser --new-window {url}"),
        web: Some(url.into()),
        group: "web".into(),
        pin: Some(pin),
        ..Default::default()
    };
    app.st.apps = vec![
        mk("brave-browser", "Brave", "Private web browser", "brave-browser %U", 1),
        mk("refurb-files", "Files", "Your documents and drives", "pcmanfm", 2),
        mk("refurb-software", "Software", "Install and remove applications", "refurb-shell --software", 3),
        mk("refurb-settings", "Settings", "Display, sound, updates and power", "refurb-shell --settings", 4),
        mk("refurb-cockpit", "Cockpit", "Web-based system administration", "brave-browser http://127.0.0.1:9090", 5),
        mk("refurb-terminal", "Terminal", "Command line", "foot", 6),
        web("m365", "Microsoft 365", "Word, Excel and PowerPoint online", "https://www.office.com", 7),
        web("outlook", "Outlook", "Mail and calendar", "https://outlook.office.com", 8),
        web("onedrive", "OneDrive", "Your files in the cloud", "https://onedrive.live.com", 9),
        web("teams", "Teams", "Meetings and chat", "https://teams.microsoft.com", 10),
        mk("refurb-calculator", "Calculator", "Do quick sums", "galculator", 11),
    ];
    app.st.pinned = crate::apps::pinned(&app.st.apps);
    app.st.toplevels = vec![
        Toplevel {
            handle: 1,
            title: "Microsoft 365 – Brave".into(),
            app_id: "brave-browser".into(),
            active: true,
        },
        Toplevel {
            handle: 2,
            title: "Documents".into(),
            app_id: "pcmanfm".into(),
            active: false,
        },
    ];
    app.st.status.wifi_present = true;
    app.st.status.wifi_enabled = true;
    app.st.status.wifi = Some(WifiStatus {
        ssid: "Home Wi-Fi".into(),
        strength: 78,
    });
    app.st.status.bt = BtStatus {
        available: true,
        powered: true,
        connected: Some("Sony WH-CH510".into()),
    };
    app.st.status.volume = 0.42;
    app.st.status.muted = false;
    app.st.status.battery = Some((0.72, false));
    app.st.status.brightness = Some(0.8);
    app.st.status.updates = 3;
    app.st.status.hostname = "reawakening-pc".into();
    app.st.status.mem_total_mb = 1962;
    app.st.status.mem_available_mb = 1180;
    app.st.status.cpu = "Intel(R) Celeron(R) N4020 CPU @ 1.10GHz".into();
    app.st.clock = "09:41".into();
    app.st.wifi.networks = vec![
        WifiNetwork {
            ssid: "Home Wi-Fi".into(),
            strength: 82,
            security: "WPA2".into(),
            active: true,
        },
        WifiNetwork {
            ssid: "Neighbour's router".into(),
            strength: 54,
            security: "WPA2".into(),
            active: false,
        },
        WifiNetwork {
            ssid: "Cafe Guest".into(),
            strength: 38,
            security: "--".into(),
            active: false,
        },
    ];
    app.st.bt.devices = vec![
        BtDevice {
            mac: "00:1B:66:11:22:33".into(),
            name: "Sony WH-CH510".into(),
            connected: true,
            paired: true,
        },
        BtDevice {
            mac: "AA:BB:CC:DD:EE:FF".into(),
            name: "Logitech K380".into(),
            connected: false,
            paired: true,
        },
    ];
    app.st.software.results = vec![
        Pkg {
            name: "firefox-esr".into(),
            summary: "Mozilla Firefox web browser - Extended Support Release".into(),
            installed: false,
        },
        Pkg {
            name: "chromium".into(),
            summary: "Web browser built on the Chromium project".into(),
            installed: false,
        },
        Pkg {
            name: "galculator".into(),
            summary: "Scientific calculator with algebraic and RPN modes".into(),
            installed: true,
        },
        Pkg {
            name: "gnome-calculator".into(),
            summary: "GNOME desktop calculator".into(),
            installed: false,
        },
        Pkg {
            name: "libreoffice-writer".into(),
            summary: "Office productivity suite - word processor".into(),
            installed: false,
        },
        Pkg {
            name: "mousepad".into(),
            summary: "Simple text editor for Xfce".into(),
            installed: false,
        },
    ];
    app.st.software.query = "calc".into();
    app.st.software.selected = Some(2);
    app.st.software.searched = true;
    app.st.display.outputs = vec![DisplayOutput {
        name: "eDP-1".into(),
        modes: vec![
            "1366x768".into(),
            "1280x720".into(),
            "1024x768".into(),
            "800x600".into(),
        ],
        current: Some("1366x768".into()),
    }];
    app.st.updates_note = vec![
        "$ apt-get update".into(),
        "Hit:1 http://deb.debian.org/debian trixie InRelease".into(),
        "Get:2 http://deb.debian.org/debian trixie-security InRelease [44.3 kB]".into(),
        "3 packages can be upgraded. 1 is a security update.".into(),
    ];
}

fn desktop_with(app: &mut App, w: u32, h: u32, overlays: &[Surface]) -> Pixmap {
    let mut base = ui::desktop_background(w, h);
    let (bar, _) = app.render(Surface::Bar, w, crate::theme::BAR_H as u32);
    paint(&mut base, &bar, 0, 0);
    for surface in overlays {
        match surface {
            Surface::Launcher => {
                let (pm, _) = app.render(Surface::Launcher, w, h);
                paint(&mut base, &pm, 0, 0);
            }
            Surface::Quick => {
                let qh = ui::quick::height(&app.st);
                let qw = ui::quick::CARD_W as u32 + 8;
                let (pm, _) = app.render(Surface::Quick, qw, qh);
                let x = w as i32 - pm.width() as i32 - 8;
                paint(&mut base, &pm, x, crate::theme::BAR_H as i32 + 8);
            }
            _ => {}
        }
    }
    base
}

fn window_with(app: &mut App, w: u32, h: u32, kind: WindowKind) -> Pixmap {
    let mut base = desktop_with(app, w, h, &[]);
    let (ww, wh) = (960u32, 620u32);
    let surface = match kind {
        WindowKind::Software => Surface::Software,
        WindowKind::Settings => Surface::Settings,
    };
    app.st.window = Some(kind);
    let (pm, _) = app.render(surface, ww, wh);
    paint(
        &mut base,
        &pm,
        ((w - ww) / 2) as i32,
        ((h - wh) / 2) as i32 + 6,
    );
    base
}

/// A contact sheet of the icon set, so every glyph can be reviewed at once.
fn icon_sheet() -> Pixmap {
    let cols = 6usize;
    let cell = 150.0f32;
    let rows = (icons::Glyph::ALL.len() + cols - 1) / cols;
    let mut pm = Pixmap::new(
        (cols as f32 * cell) as u32,
        (rows as f32 * cell) as u32 + 24,
    )
    .expect("icon sheet size");
    pm.fill(theme::card());
    let mut text = TextEngine::new();
    for (i, glyph) in icons::Glyph::ALL.iter().enumerate() {
        let x = (i % cols) as f32 * cell;
        let y = (i / cols) as f32 * cell + 20.0;
        icons::draw(
            &mut pm,
            *glyph,
            render::rect(x + 33.0, y + 8.0, 84.0, 84.0),
            theme::ink(),
            1.9,
        );
        text.text_center(
            &mut pm,
            x,
            cell,
            y + 96.0,
            18.0,
            glyph.name(),
            11.0,
            REGULAR,
            theme::ink_soft(),
        );
    }
    pm
}

/// Render every screen into `dir`; returns the files written.
pub fn render_all(dir: &Path) -> std::io::Result<Vec<PathBuf>> {
    std::fs::create_dir_all(dir)?;
    let (w, h) = (1280u32, 800u32);
    let mut app = App::new();
    seed(&mut app);
    let mut out = Vec::new();

    out.push(save(&icon_sheet(), dir.join("00-icons.png"))?);
    out.push(save(&desktop_with(&mut app, w, h, &[]), dir.join("01-desktop.png"))?);

    app.st.launcher_open = true;
    app.st.focus = Focus::Launcher;
    out.push(save(
        &desktop_with(&mut app, w, h, &[Surface::Launcher]),
        dir.join("02-launcher.png"),
    )?);

    app.st.query = "off".into();
    out.push(save(
        &desktop_with(&mut app, w, h, &[Surface::Launcher]),
        dir.join("03-launcher-search.png"),
    )?);

    app.st.query.clear();
    app.st.launcher_open = false;
    app.st.focus = Focus::None;
    app.st.quick_open = true;
    app.st.wifi.open = true;
    out.push(save(
        &desktop_with(&mut app, w, h, &[Surface::Quick]),
        dir.join("04-quick-settings.png"),
    )?);

    app.st.wifi.open = false;
    app.st.quick_open = false;
    out.push(save(
        &window_with(&mut app, w, h, WindowKind::Software),
        dir.join("05-software.png"),
    )?);

    app.st.settings.section = ui::Section::Updates;
    out.push(save(
        &window_with(&mut app, w, h, WindowKind::Settings),
        dir.join("06-settings-updates.png"),
    )?);

    app.st.settings.section = ui::Section::About;
    out.push(save(
        &window_with(&mut app, w, h, WindowKind::Settings),
        dir.join("07-settings-about.png"),
    )?);

    app.st.settings.section = ui::Section::Appearance;
    out.push(save(
        &window_with(&mut app, w, h, WindowKind::Settings),
        dir.join("09-settings-appearance.png"),
    )?);

    app.st.wifi.pending = Some("Neighbour's router".into());
    app.st.wifi.password = "hunter".into();
    app.st.quick_open = true;
    app.st.wifi.open = true;
    out.push(save(
        &desktop_with(&mut app, w, h, &[Surface::Quick]),
        dir.join("08-wifi-password.png"),
    )?);

    Ok(out)
}
