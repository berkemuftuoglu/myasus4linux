project(
    'myasus4linux',
    'rust',
    version: '0.1.0',
    meson_version: '>= 1.4',
    license: 'GPL-3.0-or-later',
)

gnome = import('gnome')

base_id = 'io.github.berkmuftuoglu.MyAsus4Linux'
is_devel = get_option('profile') == 'development'

if is_devel
    profile = 'Devel'
    application_id = '@0@.@1@'.format(base_id, profile)
else
    profile = ''
    application_id = base_id
endif

bindir = get_option('prefix') / get_option('bindir')
libexecdir = get_option('prefix') / get_option('libexecdir')
datadir = get_option('prefix') / get_option('datadir')
pkgdatadir = datadir / meson.project_name()

subdir('data')
subdir('src')

gnome.post_install(
    gtk_update_icon_cache: true,
    glib_compile_schemas: true,
    update_desktop_database: true,
)
