# Directory Structure

```
.github/
  workflows/
    ci.yml (79 lines)
crates/
  app/
    src/
      tracing/
        format.rs (87 lines)
        level.rs (25 lines)
        mod.rs (101 lines)
      app.rs (120 lines)
      diagnostics.rs (9 lines)
      lib.rs (5 lines)
      session.rs (37 lines)
    tests/
      app_test.rs (148 lines)
    Cargo.toml (39 lines)
    README.md (140 lines)
  archive/
    src/
      archive_error.rs (16 lines)
      archive.rs (280 lines)
      gz_error.rs (7 lines)
      gz.rs (80 lines)
      lib.rs (88 lines)
      tar_error.rs (7 lines)
      tar.rs (162 lines)
      tree_differ.rs (122 lines)
      zip_error.rs (7 lines)
      zip.rs (175 lines)
    tests/
      __fixtures__/
        archives/
          folder/
            nested/
              docs.md (0 lines)
              other.txt (1 lines)
            nested.json (0 lines)
            nested.txt (1 lines)
          data.json (0 lines)
          file.txt (1 lines)
      archive_test.rs (73 lines)
      gz_test.rs (39 lines)
      tar_test.rs (26 lines)
      tree_differ_test.rs (71 lines)
      utils.rs (9 lines)
      zip_test.rs (14 lines)
    Cargo.toml (61 lines)
    README.md (7 lines)
  args/
    src/
      lib.rs (354 lines)
      syntax.pest (147 lines)
    tests/
      args_test.rs (256 lines)
    Cargo.toml (12 lines)
    README.md (51 lines)
  console/
    src/
      components/
        confirm.rs (56 lines)
        entry.rs (9 lines)
        input_field.rs (19 lines)
        input.rs (38 lines)
        layout.rs (28 lines)
        list.rs (14 lines)
        map.rs (10 lines)
        mod.rs (47 lines)
        notice.rs (17 lines)
        progress.rs (213 lines)
        section.rs (5 lines)
        select.rs (119 lines)
        signal_container.rs (24 lines)
        styled_text.rs (29 lines)
        table.rs (51 lines)
      utils/
        estimator.rs (143 lines)
        formats.rs (36 lines)
        mod.rs (2 lines)
      buffer.rs (37 lines)
      console_error.rs (4 lines)
      console.rs (99 lines)
      lib.rs (12 lines)
      reporter.rs (13 lines)
      stream.rs (108 lines)
      theme.rs (90 lines)
      ui.rs (104 lines)
    Cargo.toml (26 lines)
    README.md (8 lines)
  events/
    src/
      emitter.rs (70 lines)
      event.rs (5 lines)
      lib.rs (3 lines)
      subscriber.rs (21 lines)
    tests/
      event_macros_test.rs (35 lines)
      events_test.rs (115 lines)
    Cargo.toml (19 lines)
    README.md (134 lines)
  id/
    src/
      id_error.rs (6 lines)
      id_regex.rs (21 lines)
      id.rs (165 lines)
      lib.rs (3 lines)
    tests/
      id_test.rs (43 lines)
    Cargo.toml (24 lines)
    README.md (6 lines)
  macros/
    src/
      event.rs (19 lines)
      lib.rs (25 lines)
      resource.rs (22 lines)
      state.rs (33 lines)
      subscriber.rs (72 lines)
      system.rs (204 lines)
    Cargo.toml (21 lines)
    README.md (6 lines)
  sandbox/
    src/
      fixture.rs (23 lines)
      lib.rs (11 lines)
      process.rs (129 lines)
      sandbox.rs (159 lines)
      settings.rs (32 lines)
    Cargo.toml (16 lines)
    README.md (6 lines)
  shell/
    src/
      shells/
        ash.rs (52 lines)
        bash.rs (102 lines)
        elvish.rs (159 lines)
        fish.rs (106 lines)
        ion.rs (100 lines)
        mod.rs (107 lines)
        murex.rs (118 lines)
        nu.rs (164 lines)
        powershell.rs (165 lines)
        pwsh.rs (94 lines)
        sh.rs (57 lines)
        xonsh.rs (84 lines)
        zsh.rs (67 lines)
      helpers.rs (62 lines)
      hooks.rs (6 lines)
      joiner.rs (50 lines)
      lib.rs (10 lines)
      quoter.rs (164 lines)
      shell_error.rs (3 lines)
      shell.rs (168 lines)
    tests/
      join_args_test.rs (44 lines)
      shell_test.rs (11 lines)
    Cargo.toml (35 lines)
    README.md (6 lines)
  styles/
    src/
      color.rs (240 lines)
      lib.rs (4 lines)
      stylize.rs (32 lines)
      tags.rs (113 lines)
      theme.rs (25 lines)
    tests/
      color_test.rs (42 lines)
    Cargo.toml (23 lines)
    README.md (6 lines)
  utils/
    benches/
      glob.rs (65 lines)
    src/
      envx.rs (96 lines)
      fs_error.rs (10 lines)
      fs_lock.rs (213 lines)
      fs.rs (529 lines)
      glob_cache.rs (44 lines)
      glob_error.rs (15 lines)
      glob.rs (389 lines)
      json_error.rs (14 lines)
      json.rs (140 lines)
      lib.rs (62 lines)
      net_error.rs (13 lines)
      net.rs (213 lines)
      path.rs (115 lines)
      toml_error.rs (14 lines)
      toml.rs (58 lines)
      yaml_error.rs (14 lines)
      yaml.rs (134 lines)
    tests/
      __fixtures__/
        editor-config/
          .editorconfig (4 lines)
          file.json (8 lines)
          file.yaml (7 lines)
        indent/
          spaces-4.js (1 lines)
          spaces-comments.js (9 lines)
          spaces.js (1 lines)
          tabs-2.js (1 lines)
          tabs-comments.js (9 lines)
          tabs.js (1 lines)
      fs_lock_test.rs (35 lines)
      fs_test.rs (102 lines)
      glob_test.rs (141 lines)
      json_test.rs (62 lines)
      net_test.rs (25 lines)
      yaml_test.rs (45 lines)
    Cargo.toml (78 lines)
    README.md (6 lines)
examples/
  app/
    src/
      main.rs (68 lines)
    Cargo.toml (17 lines)
  lib/
    src/
      lib.rs (13 lines)
    Cargo.toml (10 lines)
  term/
    src/
      main.rs (54 lines)
    Cargo.toml (15 lines)
.gitignore (7 lines)
.prettierignore (1 lines)
Cargo.toml (29 lines)
LICENSE (21 lines)
prettier.config.js (0 lines)
README.md (26 lines)
rust-toolchain.toml (3 lines)
```