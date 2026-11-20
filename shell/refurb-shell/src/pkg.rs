//! The software catalogue.
//!
//! Debian's own tools, asked on demand: `apt-cache` to look for packages,
//! `dpkg-query` to see what is installed, and the `refurb-apt` helper (through
//! polkit) to change the system. Nothing here runs unless the user opened the
//! Software window, and nothing stays resident afterwards.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::Sender;

use crate::ui::Pkg;

/// Progress of a long package operation.
pub enum Event {
    Line(String),
    Done(bool),
}

fn run(prog: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(prog)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Category chips in the Software window map to descriptive searches.
pub fn category_term(query: &str) -> Option<&'static str> {
    Some(match query.trim() {
        "Internet" => "web browser",
        "Office" => "office suite",
        "Graphics" => "image editor",
        "Education" => "education",
        "Utilities" => "utility",
        _ => return None,
    })
}

/// Is this a plausible Debian package name? Used to keep anything that came
/// from the interface out of the shell command and out of apt's arguments.
pub fn valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.' | '_'))
        && name.starts_with(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit())
}

pub fn installed_set() -> HashSet<String> {
    let mut out = HashSet::new();
    let Some(text) = run(
        "dpkg-query",
        &["-W", "-f=${db:Status-Abbrev}\t${binary:Package}\n"],
    ) else {
        return out;
    };
    for line in text.lines() {
        let mut parts = line.splitn(2, '\t');
        let (Some(status), Some(name)) = (parts.next(), parts.next()) else {
            continue;
        };
        if status.trim().starts_with("ii") {
            out.insert(name.trim().to_string());
        }
    }
    out
}

/// Search the catalogue. Results are alphabetical with exact-prefix matches
/// first, and tagged with the installed state.
pub fn search(query: &str, limit: usize) -> Vec<Pkg> {
    let query = query.trim();
    if query.len() < 2 {
        return Vec::new();
    }
    let installed = installed_set();
    let (term, descriptive) = match category_term(query) {
        Some(term) => (term.to_string(), true),
        None => (query.to_string(), false),
    };
    let mut args: Vec<&str> = vec!["search"];
    if !descriptive {
        args.push("--names-only");
    }
    args.push(&term);
    let Some(text) = run("apt-cache", &args) else {
        return Vec::new();
    };

    let needle = query.to_lowercase();
    let mut hits: Vec<Pkg> = Vec::new();
    collect(&text, &installed, &mut hits, limit * 3);
    // A package name is not how people search. "video player", "photo
    // editor" and "jellyfin" are descriptions, and `apt-cache search` covers
    // them when `--names-only` is dropped. The name search still runs first so
    // a package called `firefox-esr` is never buried under its description.
    if hits.len() < 3 && !descriptive {
        if let Some(text) = run("apt-cache", &["search", &term]) {
            collect(&text, &installed, &mut hits, limit * 3);
        }
    }
    hits.sort_by(|a, b| {
        let rank = |p: &Pkg| {
            let n = p.name.to_lowercase();
            if n == needle {
                0
            } else if n.starts_with(&needle) {
                1
            } else {
                2
            }
        };
        rank(a)
            .cmp(&rank(b))
            .then_with(|| a.name.len().cmp(&b.name.len()))
            .then_with(|| a.name.cmp(&b.name))
    });
    hits.truncate(limit);
    hits
}

/// Add the `name - summary` lines of an `apt-cache` reply, skipping anything
/// already listed or not a valid Debian package name.
fn collect(text: &str, installed: &HashSet<String>, hits: &mut Vec<Pkg>, cap: usize) {
    for line in text.lines() {
        let Some((name, summary)) = line.split_once(" - ") else {
            continue;
        };
        let name = name.trim();
        if !valid_package_name(name) {
            continue;
        }
        if hits.iter().any(|p| p.name == name) {
            continue;
        }
        hits.push(Pkg {
            name: name.to_string(),
            summary: summary.trim().to_string(),
            installed: installed.contains(name),
        });
        if hits.len() >= cap {
            return;
        }
    }
}

/// Install or remove a package through the privileged helper, streaming the
/// output back line by line. Runs on a worker thread.
pub fn run_action(install: bool, package: &str, tx: Sender<Event>) {
    if !valid_package_name(package) {
        let _ = tx.send(Event::Line(format!("Refusing suspicious package name: {package}")));
        let _ = tx.send(Event::Done(false));
        return;
    }
    let verb = if install { "install" } else { "remove" };
    // The helper is the only thing the session user may run as root; the
    // sudoers rule is limited to this exact path (see distro/includes).
    let mut child = match Command::new("sudo")
        .args(["-n", "/usr/lib/refurb/refurb-apt", verb, package])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = tx.send(Event::Line(format!("Could not start the helper: {e}")));
            let _ = tx.send(Event::Line(
                "refurb-apt is installed by the Reawakening OS base package.".into(),
            ));
            let _ = tx.send(Event::Done(false));
            return;
        }
    };
    let mut ok = true;
    if let Some(err) = child.stderr.take() {
        for line in BufReader::new(err).lines().map_while(Result::ok) {
            let _ = tx.send(Event::Line(line));
        }
    }
    if let Some(out) = child.stdout.take() {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let _ = tx.send(Event::Line(line));
        }
    }
    if let Ok(status) = child.wait() {
        ok = status.success();
    }
    let _ = tx.send(Event::Done(ok));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_names_that_could_be_flags_or_shell() {
        assert!(valid_package_name("firefox-esr"));
        assert!(valid_package_name("g++"));
        assert!(valid_package_name("libreoffice-writer"));
        assert!(!valid_package_name("--assume-yes"));
        assert!(!valid_package_name("rm -rf /"));
        assert!(!valid_package_name("Firefox"));
        assert!(!valid_package_name(""));
    }

    #[test]
    fn categories_map_to_descriptive_searches() {
        assert_eq!(category_term("Internet"), Some("web browser"));
        assert_eq!(category_term("calc"), None);
    }
}
