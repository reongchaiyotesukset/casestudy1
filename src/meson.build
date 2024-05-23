pkgdatadir = join_paths(get_option('prefix'), get_option('datadir'), meson.project_name())

global_conf = configuration_data()
global_conf.set_quoted('APP_ID', application_id)
global_conf.set_quoted('PKGDATADIR', pkgdatadir)
global_conf.set_quoted('PROFILE', profile)
global_conf.set_quoted('VERSION', version + version_suffix)
global_conf.set_quoted('GETTEXT_PACKAGE', gettext_package)
global_conf.set_quoted('LOCALEDIR', localedir)
global_conf.set_quoted('OBJECT_PATH', object_path)
config_output_file = configure_file(
    input: 'config.rs.in',
    output: 'config.rs',
    configuration: global_conf
)

# Copy the config.rs output to the source directory.
run_command(
  'cp',
  join_paths(meson.project_build_root(), 'src', 'config.rs'),
  join_paths(meson.project_source_root(), 'src', 'config.rs'),
  check: true
)

cargo_options = [ '--manifest-path', meson.project_source_root() / 'Cargo.toml' ]
cargo_options += [ '--target-dir', meson.project_build_root() / 'src' ]

if get_option('profile') == 'default'
  cargo_options += [ '--release' ]
  rust_target = 'release'
  message('Building in release mode')
else
  rust_target = 'debug'
  message('Building in debug mode')
endif

cargo_env = [ 'CARGO_HOME=' + meson.project_build_root() / 'cargo-home' ]

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  depends: resources,
  command: [
    'env',
    cargo_env,
    cargo, 'build',
    cargo_options,
    '&&',
    'cp', 'src' / rust_target / meson.project_name(), '@OUTPUT@',
  ]
)

cargo_target_dir = meson.project_build_root() / 'target'
cargo_home = cargo_target_dir / 'cargo-home'
test (
  'Cargo tests',
  cargo,
  args: ['test', '--bin', meson.project_name()],
  timeout: 600, # cargo might take a bit of time sometimes
  env: ['CARGO_TARGET_DIR=@0@'.format(cargo_target_dir), 'CARGO_HOME=@0@'.format(cargo_home)]
)

doc_deps = [
  '--package=favicon-scrapper',
  '--package=authenticator'
]
custom_target(
  'cargo-doc',
  build_by_default: false,
  build_always_stale: true,
  output: 'doc',
  console: true,
  command: [
    'env', cargo_env,
    cargo, 'doc',
    cargo_options + doc_deps + ['--no-deps', '--document-private-items'] ,
  ],
)
