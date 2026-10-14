#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(any(windows, test))]
mod geometry;
#[cfg(any(windows, test))]
mod png;
#[cfg(any(windows, test))]
mod annotations;
#[cfg(any(windows, test))]
mod selection;
#[cfg(any(windows, test))]
mod toolbar;
#[cfg(windows)]
mod windows;

#[cfg(windows)]
fn main() {
    windows::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("RustShot GUI runs on Windows 11 only. Platform-independent unit tests can run anywhere.");
}
