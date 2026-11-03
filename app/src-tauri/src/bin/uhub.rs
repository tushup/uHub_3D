//! `uhub` CLI entry point. All logic lives in `uhub3d_lib::cli`.

fn main() {
    std::process::exit(uhub3d_lib::cli::run());
}
