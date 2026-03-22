/// Application ID, set at build time by Meson or falling back to the Devel ID.
pub const APP_ID: &str = match option_env!("APP_ID") {
    Some(v) => v,
    None => "io.github.berkmuftuoglu.MyAsus4Linux.Devel",
};

/// Path to the compiled GResource bundle, set at build time by Meson.
pub const RESOURCES_FILE: &str = match option_env!("RESOURCES_FILE") {
    Some(v) => v,
    None => "data/resources/resources.gresource",
};
