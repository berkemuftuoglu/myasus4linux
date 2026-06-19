subdir('resources')

if host_machine.system() == 'linux'
    subdir('icons')

    # udev rule so the safe controls (kbd backlight, fan profile, charge limit)
    # are user-writable and don't trigger a polkit prompt on every change.
    install_data(
        '99-myasus4linux.rules',
        install_dir: get_option('prefix') / 'lib' / 'udev' / 'rules.d',
    )

    desktop_conf = configuration_data()
    desktop_conf.set('APP_ID', application_id)
    configure_file(
        input: '@0@.desktop.in.in'.format(base_id),
        output: '@0@.desktop'.format(application_id),
        configuration: desktop_conf,
        install: true,
        install_dir: datadir / 'applications',
    )

    gschema_conf = configuration_data()
    gschema_conf.set('APP_ID', application_id)
    configure_file(
        input: '@0@.gschema.xml.in'.format(base_id),
        output: '@0@.gschema.xml'.format(application_id),
        configuration: gschema_conf,
        install: true,
        install_dir: datadir / 'glib-2.0' / 'schemas',
    )

    metainfo_conf = configuration_data()
    metainfo_conf.set('APP_ID', application_id)
    metainfo_conf.set('APP_NAME', meson.project_name())
    configure_file(
        input: '@0@.metainfo.xml.in.in'.format(base_id),
        output: '@0@.metainfo.xml'.format(application_id),
        configuration: metainfo_conf,
        install: true,
        install_dir: datadir / 'metainfo',
    )
endif
