mod app;
mod capture;
mod appicon;
mod config;
mod control;
mod defaults;
mod gfx;
mod calc;
mod icons;
mod keybinds;
mod mpris;
mod idle;
mod launch;
mod limits;
mod notif;
mod sys;
mod theme;
mod toplevel;
mod tray;
mod ui;
mod win;
mod workspace;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,zbus=warn,wgpu_hal=warn,wgpu_core=warn,naga=warn")).init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        if args[0] == "--version" || args[0] == "version" {
            println!("osmia {}", env!("CARGO_PKG_VERSION"));
            return;
        }
        // Capture works without the shell running.
        if args[0] == "grab" {
            std::process::exit(capture::cli(&args[1..]));
        }
        std::process::exit(control::client(&args));
    }
    // Ask logind in the background, so the power menu knows whether to offer Hibernate.
    sys::can_hibernate();
    if let Err(e) = app::run() {
        eprintln!("osmia: {e}");
        std::process::exit(1);
    }
}
