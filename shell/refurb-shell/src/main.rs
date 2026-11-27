//! Reawakening OS shell.
//!
//! One process draws the wallpaper, the bar, the launcher, quick settings and
//! Settings. It is a Wayland client of the labwc compositor and owns no
//! background work of its own.

mod apps;
mod desktop;
mod displays;
mod icons;
mod keys;
mod prefs;
mod preview;
mod reaper;
mod render;
mod system;
mod text;
mod theme;
mod ui;
mod wallpaper;

#[cfg(feature = "session")]
mod session;

const USAGE: &str = "\
refurb-shell — the Reawakening OS desktop shell

USAGE:
  refurb-shell                     run the desktop session (needs Wayland + labwc)
  refurb-shell --render-preview DIR
                                   render every screen to PNG files in DIR
  refurb-shell --send COMMAND      tell a running shell what to do
                                   (launcher, quick, settings, display, identify,
                                    lock, power, quit)
  refurb-shell --wallpaper-set FILE
                                   use a picture as the desktop wallpaper; the
                                   picture is copied into the user's own
                                   collection first, so the original can move
  refurb-shell --version
  refurb-shell --help

The shell is a single process: no tray, no indexer, no notification daemon.
";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version") | Some("-V") => {
            println!("refurb-shell {}", env!("CARGO_PKG_VERSION"));
        }
        Some("--render-preview") => {
            let dir = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| "preview".to_string());
            match preview::render_all(std::path::Path::new(&dir)) {
                Ok(files) => {
                    for f in files {
                        println!("{}", f.display());
                    }
                }
                Err(e) => {
                    eprintln!("refurb-shell: could not render preview: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("--wallpaper-set") => {
            let Some(source) = args.get(1) else {
                eprintln!("refurb-shell: --wallpaper-set needs an image file");
                std::process::exit(2);
            };
            match wallpaper::install(std::path::Path::new(source)).and_then(|wall| {
                wallpaper::activate(&wall).map_err(|e| e.to_string())?;
                Ok(wall)
            }) {
                Ok(wall) => println!("Wallpaper set to {}", wall.path().display()),
                Err(e) => {
                    eprintln!("refurb-shell: could not set the wallpaper: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some("--send") => {
            let command = args.get(1).cloned().unwrap_or_default();
            #[cfg(feature = "session")]
            {
                if let Err(e) = session::send_command(&command) {
                    eprintln!("refurb-shell: {e}");
                    std::process::exit(1);
                }
            }
            #[cfg(not(feature = "session"))]
            {
                let _ = command;
                eprintln!("refurb-shell: this build has no session support");
                std::process::exit(2);
            }
        }
        Some("--help") | Some("-h") => print!("{USAGE}"),
        Some(other) => {
            eprintln!("refurb-shell: unknown argument {other}\n\n{USAGE}");
            std::process::exit(2);
        }
        None => {
            #[cfg(feature = "session")]
            {
                if let Err(e) = session::run() {
                    eprintln!("refurb-shell: {e}");
                    std::process::exit(1);
                }
            }
            #[cfg(not(feature = "session"))]
            {
                eprintln!(
                    "refurb-shell: this build has no session support.\n\
                     Rebuild with the default features (Wayland client) to run a desktop."
                );
                std::process::exit(2);
            }
        }
    }
}
