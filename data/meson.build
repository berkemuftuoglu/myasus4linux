subdir('resources')

if host_machine.system() == 'linux'
    subdir('icons')

    # The ASUS fan/charge/keyboard controls are written by the myasusd daemon
    # over D-Bus, gated by polkit. The screen backlight gets a uaccess rule so
    # the active session can dim the display without root.
    install_data(
        '99-myasus4linux-backlight.rules',
        install_dir: get_option('prefix') / 'lib' / 'udev' / 'rules.d',
    )
    install_data(
        'io.github.berkmuftuoglu.MyAsus4Linux.Helper.conf',
        install_dir: datadir / 'dbus-1' / 'system.d',
    )
    install_data(
        'io.github.berkmuftuoglu.MyAsus4Linux.Helper.service',
        install_dir: datadir / 'dbus-1' / 'system-services',
    )
    install_data(
        'myasusd.service',
        install_dir: get_option('prefix') / 'lib' / 'systemd' / 'system',
    )
    install_data(
        '../polkit/io.github.berkmuftuoglu.MyAsus4Linux.Helper.policy',
        install_dir: datadir / 'polkit-1' / 'actions',
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
