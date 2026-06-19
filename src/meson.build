cargo = find_program('cargo')
# Build the whole workspace so the daemon (myasusd) is produced alongside the GUI.
cargo_options = ['--manifest-path', meson.project_source_root() / 'Cargo.toml']
cargo_options += ['--target-dir', meson.project_build_root() / 'target']
cargo_options += ['--workspace']

if not is_devel
    cargo_options += ['--release']
    rust_target = 'release'
else
    rust_target = 'debug'
endif

target_dir = meson.project_build_root() / 'target' / rust_target

gui = custom_target(
    'cargo-build',
    build_by_default: true,
    build_always_stale: true,
    output: meson.project_name(),
    console: true,
    install: true,
    install_dir: bindir,
    depends: resources,
    env: {
        'CARGO_HOME': meson.project_build_root() / 'cargo-home',
        'APP_ID': application_id,
        'RESOURCES_FILE': pkgdatadir / 'resources.gresource',
    },
    command: [
        cargo, 'build', cargo_options,
        '&&', 'cp', target_dir / meson.project_name(), '@OUTPUT@',
    ],
)

# The privileged D-Bus daemon, installed to libexec. The workspace build above
# already produces it, so this just stages the binary for install.
custom_target(
    'cargo-build-daemon',
    build_by_default: true,
    output: 'myasusd',
    console: true,
    install: true,
    install_dir: libexecdir,
    depends: gui,
    command: ['cp', target_dir / 'myasusd', '@OUTPUT@'],
)
