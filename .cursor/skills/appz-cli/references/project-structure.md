# Directory Structure

```
.agents/
  skills/
    domain-cli/
      SKILL.md (161 lines)
    m01-ownership/
      examples/
        best-practices.md (339 lines)
      patterns/
        common-errors.md (265 lines)
        lifetime-patterns.md (229 lines)
      comparison.md (222 lines)
      SKILL.md (134 lines)
    m03-mutability/
      SKILL.md (153 lines)
    m04-zero-cost/
      SKILL.md (165 lines)
    m06-error-handling/
      examples/
        library-vs-app.md (332 lines)
      patterns/
        error-patterns.md (404 lines)
      SKILL.md (166 lines)
    m07-concurrency/
      examples/
        thread-patterns.md (396 lines)
      patterns/
        async-patterns.md (409 lines)
        common-errors.md (331 lines)
      comparison.md (312 lines)
      SKILL.md (222 lines)
    m10-performance/
      patterns/
        optimization-guide.md (365 lines)
      SKILL.md (157 lines)
    m13-domain-error/
      SKILL.md (180 lines)
    m15-anti-pattern/
      patterns/
        common-mistakes.md (421 lines)
      SKILL.md (160 lines)
    meta-cognition-parallel/
      SKILL.md (352 lines)
.github/
  workflows/
    copr-publish.yml (98 lines)
    npm-publish.yml (101 lines)
    ppa-publish.yml (188 lines)
    release-prepare.yml (56 lines)
    release-tag.yml (66 lines)
    release.yml (260 lines)
    snapcraft-publish.yml (26 lines)
    winget.yml (25 lines)
  RELEASE_SETUP.md (100 lines)
chocolatey/
  appz/
    tools/
      chocolateyInstall.ps1 (37 lines)
      chocolateyUninstall.ps1 (22 lines)
    appz.nuspec (22 lines)
crates/
  ai/
    src/
      skills/
        mod.rs (128 lines)
        tool.rs (49 lines)
      aisdk_client.rs (153 lines)
      client.rs (374 lines)
      error.rs (12 lines)
      lib.rs (34 lines)
      parse.rs (111 lines)
      types.rs (135 lines)
    Cargo.toml (22 lines)
  api/
    src/
      endpoints/
        aliases.rs (34 lines)
        auth.rs (131 lines)
        deployments.rs (53 lines)
        domains.rs (31 lines)
        gen.rs (20 lines)
        mod.rs (19 lines)
        plugins.rs (23 lines)
        projects.rs (32 lines)
        teams.rs (76 lines)
        users.rs (12 lines)
      http/
        error_mapper.rs (16 lines)
        response_handler.rs (96 lines)
      middleware/
        auth.rs (102 lines)
        mod.rs (4 lines)
        retry.rs (67 lines)
        team.rs (23 lines)
        tracing.rs (46 lines)
      model/
        types.rs (0 lines)
      client.rs (379 lines)
      error.rs (4 lines)
      lib.rs (58 lines)
      models.rs (90 lines)
    API_ENDPOINT_RULES.md (447 lines)
    Cargo.toml (22 lines)
    README.md (111 lines)
  app/
    src/
      commands/
        aliases/
          ls.rs (54 lines)
          mod.rs (91 lines)
          rm.rs (55 lines)
        domains/
          ls.rs (51 lines)
          mod.rs (40 lines)
          rm.rs (47 lines)
        projects/
          add.rs (51 lines)
          ls.rs (57 lines)
          mod.rs (112 lines)
          rm.rs (57 lines)
        promote/
          mod.rs (76 lines)
          status.rs (146 lines)
        rollback/
          mod.rs (72 lines)
          status.rs (146 lines)
        skills/
          add.rs (129 lines)
          audit.rs (219 lines)
          list.rs (70 lines)
          mod.rs (82 lines)
          remove.rs (54 lines)
          validate.rs (214 lines)
        teams/
          add.rs (171 lines)
          invite.rs (40 lines)
          list.rs (71 lines)
          mod.rs (118 lines)
          rm.rs (56 lines)
          switch.rs (59 lines)
        build.rs (254 lines)
        check.rs (137 lines)
        deploy.rs (397 lines)
        deployment_utils.rs (101 lines)
        dev_server.rs (44 lines)
        dev.rs (177 lines)
        external.rs (84 lines)
        gen.rs (56 lines)
        init.rs (93 lines)
        link.rs (34 lines)
        list.rs (23 lines)
        login.rs (53 lines)
        logout.rs (34 lines)
        ls.rs (45 lines)
        mcp_server.rs (8 lines)
        migrate.rs (26 lines)
        mod.rs (72 lines)
        plan.rs (13 lines)
        plugin.rs (60 lines)
        preview.rs (106 lines)
        recipe_validate.rs (16 lines)
        remove.rs (188 lines)
        run.rs (34 lines)
        self_upgrade_stub.rs (37 lines)
        self_upgrade.rs (141 lines)
        site.rs (240 lines)
        switch.rs (59 lines)
        unlink.rs (23 lines)
        version.rs (11 lines)
      detectors/
        detect.rs (409 lines)
        filesystem.rs (52 lines)
        mod.rs (4 lines)
        README.md (133 lines)
      project/
        config.rs (59 lines)
        connect_git.rs (53 lines)
        edit_project_settings.rs (151 lines)
        humanize_path.rs (32 lines)
        input_project.rs (126 lines)
        input_root_directory.rs (67 lines)
        select_org.rs (78 lines)
      recipe/
        deploy/
          cleanup.rs (8 lines)
          clear_paths.rs (28 lines)
          copy_dirs.rs (75 lines)
          env.rs (9 lines)
          info.rs (7 lines)
          lock.rs (32 lines)
          mod.rs (12 lines)
          release.rs (30 lines)
          setup.rs (16 lines)
          shared.rs (12 lines)
          symlink.rs (16 lines)
          update_code.rs (12 lines)
          writable.rs (16 lines)
        tools/
          ddev/
            common.rs (67 lines)
            install.rs (78 lines)
            mkcert.rs (6 lines)
            mod.rs (27 lines)
            uninstall.rs (86 lines)
            verify.rs (16 lines)
          docker/
            common.rs (48 lines)
            install.rs (184 lines)
            mod.rs (17 lines)
            verify.rs (17 lines)
          mise.rs (119 lines)
          mod.rs (16 lines)
        common.rs (32 lines)
        laravel.rs (87 lines)
        mod.rs (39 lines)
        test.rs (21 lines)
        vercel.rs (24 lines)
      services/
        mod.rs (2 lines)
        template_error.rs (4 lines)
        template.rs (414 lines)
      systems/
        analyze.rs (2 lines)
        bootstrap.rs (42 lines)
        execute.rs (2 lines)
        mod.rs (5 lines)
        startup.rs (36 lines)
        version_check.rs (188 lines)
      templates/
        mod.rs (45 lines)
      utils/
        ext.rs (0 lines)
        fs.rs (32 lines)
        json.rs (1 lines)
        mod.rs (3 lines)
      wasm/
        host_functions/
          context.rs (48 lines)
          execution.rs (94 lines)
          filesystem.rs (31 lines)
          helpers.rs (36 lines)
          host.rs (41 lines)
          interaction.rs (46 lines)
          mod.rs (17 lines)
          plugin_ast.rs (46 lines)
          plugin_check.rs (19 lines)
          plugin_fs.rs (67 lines)
          plugin_git.rs (27 lines)
          plugin_sandbox.rs (27 lines)
          plugin_site.rs (15 lines)
          registry.rs (61 lines)
          stubs.rs (34 lines)
          utils.rs (72 lines)
        mod.rs (6 lines)
        plugin.rs (336 lines)
        types.rs (223 lines)
      app_error.rs (4 lines)
      app.rs (259 lines)
      appz-cli.code-workspace (11 lines)
      auth.rs (274 lines)
      context.rs (43 lines)
      host.rs (140 lines)
      http.rs (196 lines)
      importer.rs (460 lines)
      lib.rs (22 lines)
      log.rs (5 lines)
      project.rs (586 lines)
      sandbox_helpers.rs (35 lines)
      session.rs (247 lines)
      shell.rs (447 lines)
      ssh.rs (124 lines)
      tunnel.rs (216 lines)
    Cargo.toml (82 lines)
  appz_pdk/
    src/
      lib.rs (117 lines)
      macros.rs (77 lines)
      plugin_macros.rs (94 lines)
      prelude.rs (17 lines)
      security.rs (68 lines)
      types.rs (258 lines)
    Cargo.toml (22 lines)
    README.md (155 lines)
  checker/
    src/
      ai_fixer/
        agents.rs (279 lines)
        context.rs (368 lines)
        llm.rs (27 lines)
        mod.rs (416 lines)
        patch.rs (438 lines)
        safety.rs (135 lines)
      providers/
        biome.rs (174 lines)
        clippy.rs (168 lines)
        helpers.rs (83 lines)
        mod.rs (18 lines)
        phpstan.rs (162 lines)
        ruff.rs (206 lines)
        secrets.rs (171 lines)
        stylelint.rs (179 lines)
        typescript.rs (126 lines)
      cache.rs (120 lines)
      config.rs (293 lines)
      detect.rs (160 lines)
      error.rs (38 lines)
      fixer.rs (138 lines)
      git.rs (88 lines)
      init.rs (75 lines)
      lib.rs (166 lines)
      output.rs (190 lines)
      provider.rs (237 lines)
      runner.rs (283 lines)
    Cargo.toml (29 lines)
  cli/
    src/
      main.rs (84 lines)
    Cargo.toml (40 lines)
  command/
    src/
      error.rs (4 lines)
      exec.rs (247 lines)
      lib.rs (7 lines)
      shell.rs (66 lines)
    Cargo.toml (10 lines)
  common/
    src/
      consts.rs (102 lines)
      env.rs (70 lines)
      hardening.rs (86 lines)
      head_tail_buffer.rs (196 lines)
      id.rs (101 lines)
      lib.rs (22 lines)
      path.rs (148 lines)
      types.rs (32 lines)
      user_config.rs (280 lines)
    Cargo.toml (17 lines)
    README.md (35 lines)
  deployer/
    src/
      providers/
        aws_s3.rs (146 lines)
        azure_static.rs (162 lines)
        cloudflare_pages.rs (154 lines)
        firebase.rs (173 lines)
        fly.rs (165 lines)
        github_pages.rs (174 lines)
        helpers.rs (74 lines)
        mod.rs (22 lines)
        netlify.rs (184 lines)
        render.rs (124 lines)
        surge.rs (147 lines)
        vercel.rs (214 lines)
      config.rs (397 lines)
      detect.rs (178 lines)
      error.rs (36 lines)
      lib.rs (208 lines)
      output.rs (131 lines)
      provider.rs (259 lines)
      ui.rs (109 lines)
    Cargo.toml (21 lines)
  dev-server/
    src/
      handlers/
        form_data.rs (107 lines)
        mod.rs (7 lines)
        static_files.rs (206 lines)
        websocket.rs (70 lines)
      config.rs (50 lines)
      error.rs (10 lines)
      lib.rs (10 lines)
      server.rs (129 lines)
      watcher.rs (72 lines)
    Cargo.toml (23 lines)
    README.md (153 lines)
  env_var/
    src/
      env_scanner.rs (26 lines)
      env_substitutor.rs (104 lines)
      global_bag.rs (88 lines)
      lib.rs (3 lines)
    Cargo.toml (11 lines)
  frameworks/
    data/
      frameworks.json (1088 lines)
      php-frameworks.json (250 lines)
    src/
      frameworks.rs (83 lines)
      lib.rs (2 lines)
      types.rs (58 lines)
    .gitignore (3 lines)
    build.rs (158 lines)
    Cargo.toml (15 lines)
    README.md (119 lines)
  init/
    src/
      providers/
        framework.rs (107 lines)
        git.rs (60 lines)
        local.rs (37 lines)
        mod.rs (6 lines)
        npm.rs (32 lines)
        remote_archive.rs (28 lines)
      sources/
        download.rs (54 lines)
        git.rs (203 lines)
        mod.rs (5 lines)
        npm.rs (91 lines)
        remote_archive.rs (40 lines)
      config.rs (86 lines)
      detect.rs (145 lines)
      error.rs (19 lines)
      lib.rs (107 lines)
      output.rs (14 lines)
      provider.rs (76 lines)
      run.rs (117 lines)
      ui.rs (56 lines)
    Cargo.toml (25 lines)
  mcp-server/
    src/
      auth.rs (35 lines)
      lib.rs (15 lines)
      main.rs (12 lines)
      tools.rs (269 lines)
    Cargo.toml (28 lines)
    README.md (71 lines)
  plugin-build/
    src/
      lib.rs (298 lines)
      main.rs (99 lines)
      wasm_header.rs (76 lines)
    Cargo.toml (25 lines)
  plugin-manager/
    src/
      cache.rs (75 lines)
      downloader.rs (62 lines)
      entitlements.rs (114 lines)
      error.rs (23 lines)
      lib.rs (274 lines)
      manifest.rs (140 lines)
      security.rs (287 lines)
      update_check.rs (108 lines)
    Cargo.toml (42 lines)
  plugins/
    check/
      src/
        lib.rs (84 lines)
      Cargo.toml (15 lines)
    site/
      src/
        lib.rs (110 lines)
      Cargo.toml (15 lines)
    ssg-migrator/
      src/
        lib.rs (286 lines)
        vfs_wasm.rs (132 lines)
      Cargo.toml (21 lines)
    Cargo.toml (2 lines)
  sandbox/
    src/
      local/
        mod.rs (399 lines)
      config.rs (260 lines)
      error.rs (158 lines)
      json_ops.rs (253 lines)
      lib.rs (381 lines)
      mise.rs (393 lines)
      provider.rs (435 lines)
      scoped_fs.rs (990 lines)
      toml_ops.rs (172 lines)
    Cargo.toml (27 lines)
  site-builder/
    src/
      firecrawl/
        mod.rs (164 lines)
        types.rs (131 lines)
      pipeline/
        analyze.rs (161 lines)
        assets.rs (100 lines)
        build.rs (33 lines)
        crawl.rs (22 lines)
        generate.rs (356 lines)
        mod.rs (155 lines)
        scaffold.rs (156 lines)
      prompts/
        assemble.rs (36 lines)
        classify.rs (18 lines)
        ia.rs (14 lines)
        mod.rs (5 lines)
        transform.rs (9 lines)
      templates/
        components/
          card_grid.astro (52 lines)
          cta.astro (42 lines)
          footer.astro (67 lines)
          hero.astro (119 lines)
          navbar.astro (80 lines)
          section.astro (42 lines)
          stats.astro (35 lines)
          testimonial.astro (33 lines)
        base_layout.astro (38 lines)
        mod.rs (36 lines)
      themes/
        presets/
          corporate.rs (23 lines)
          minimal.rs (22 lines)
          mod.rs (10 lines)
          nonprofit.rs (22 lines)
          startup.rs (23 lines)
        css.rs (28 lines)
        mod.rs (82 lines)
        tailwind.rs (13 lines)
      cache.rs (90 lines)
      config.rs (101 lines)
      error.rs (12 lines)
      lib.rs (18 lines)
    Cargo.toml (25 lines)
  ssg-migrator/
    src/
      generator/
        classifier.rs (84 lines)
        components.rs (75 lines)
        config.rs (69 lines)
        files.rs (37 lines)
        fix_imports.rs (57 lines)
        layout.rs (16 lines)
        mod.rs (97 lines)
        pages.rs (41 lines)
        readme.rs (13 lines)
        regex.rs (35 lines)
        templates.rs (3 lines)
        transform.rs (181 lines)
      nextjs/
        config.rs (80 lines)
        convert.rs (152 lines)
        mod.rs (59 lines)
        pages.rs (67 lines)
        providers.rs (33 lines)
        regex.rs (25 lines)
        templates.rs (1 lines)
        transform.rs (152 lines)
        verify.rs (91 lines)
      sync/
        backward.rs (32 lines)
        forward.rs (46 lines)
        mod.rs (144 lines)
      analyzer.rs (222 lines)
      ast_transformer.rs (127 lines)
      common.rs (51 lines)
      lib.rs (25 lines)
      transformer.rs (95 lines)
      types.rs (35 lines)
      vfs_native.rs (136 lines)
      vfs.rs (54 lines)
    Cargo.toml (36 lines)
  studio/
    src/
      apply.rs (61 lines)
      lib.rs (18 lines)
      main.rs (2 lines)
      parse.rs (58 lines)
      scaffold.rs (32 lines)
    Cargo.toml (13 lines)
  task/
    src/
      context.rs (140 lines)
      deps.rs (286 lines)
      error.rs (6 lines)
      lib.rs (18 lines)
      registry.rs (121 lines)
      runner.rs (393 lines)
      scheduler.rs (222 lines)
      source_tracker.rs (203 lines)
      task_executor.rs (248 lines)
      task.rs (132 lines)
      types.rs (27 lines)
    Cargo.toml (18 lines)
  ui/
    src/
      banner.rs (100 lines)
      empty.rs (52 lines)
      error.rs (79 lines)
      format.rs (237 lines)
      layout.rs (64 lines)
      lib.rs (23 lines)
      list.rs (92 lines)
      pagination.rs (94 lines)
      progress.rs (254 lines)
      prompt.rs (234 lines)
      status.rs (84 lines)
      table.rs (180 lines)
    Cargo.toml (15 lines)
    README.md (98 lines)
docs/
  developer/
    api.md (366 lines)
    architecture.md (330 lines)
    contributing.md (280 lines)
    extending.md (457 lines)
    mise-architecture-analysis.md (431 lines)
    mise-env-variables.md (246 lines)
    mise-vs-saasctl-comparison.md (251 lines)
    petgraph-vs-kahns-comparison.md (277 lines)
    testing-workflows.md (160 lines)
    testing.md (450 lines)
    wasm-plugin-debugging.md (42 lines)
    wasm-plugin-fix-summary.md (74 lines)
    wasm-plugin-hyper-mcp-analysis.md (76 lines)
    wasm-plugin-improvements.md (574 lines)
    wasm-plugin-troubleshooting.md (95 lines)
    wasm-plugin-webxtism-analysis.md (84 lines)
  user/
    advanced.md (396 lines)
    context.md (272 lines)
    getting-started.md (138 lines)
    guide.md (342 lines)
    recipes.md (277 lines)
  README.md (23 lines)
examples/
  .appz/
    tasks/
      common.yaml (36 lines)
  plugins/
    hello/
      src/
        lib.rs (96 lines)
      Cargo.toml (15 lines)
      README.md (23 lines)
  recipes/
    common.yaml (35 lines)
    deploy.yaml (21 lines)
  comprehensive-recipe.yaml (193 lines)
packages/
  appz-mcp-server/
    bin/
      mcp-server.js (7 lines)
    package.json (28 lines)
    README.md (60 lines)
packaging/
  appz.run/
    shell.envsubst (57 lines)
  standalone/
    install.envsubst (280 lines)
    install.ps1.envsubst (173 lines)
recipes/
  aws-s3.yaml (55 lines)
  azure-static.yaml (59 lines)
  cloudflare-pages.yaml (61 lines)
  firebase.yaml (65 lines)
  fly.yaml (65 lines)
  github-pages.yaml (53 lines)
  heroku.yaml (67 lines)
  netlify.yaml (59 lines)
  railway.yaml (61 lines)
  README.md (486 lines)
  render.yaml (57 lines)
  surge.yaml (43 lines)
  vercel.yaml (63 lines)
scripts/
  build-tarball.ps1 (94 lines)
  build-tarball.sh (113 lines)
  generate-plugin-signing-key.sh (39 lines)
  generate-test-keys.ps1 (34 lines)
  generate-test-keys.sh (24 lines)
  get-version.ps1 (26 lines)
  get-version.sh (13 lines)
  Makefile.plugins (25 lines)
  plugins.toml (36 lines)
  README-plugin-build.md (115 lines)
  release-prepare.sh (114 lines)
  render-appz-run.sh (45 lines)
  render-install-ps1.sh (9 lines)
  render-install.sh (23 lines)
  setup-zipsign.sh (14 lines)
  test-workflow-linux.sh (24 lines)
  test-workflow-windows.ps1 (33 lines)
test-keys/
  test_key.pub (1 lines)
winget/
  appz.yaml (35 lines)
.gitignore (59 lines)
appz-cli.code-workspace (17 lines)
Cargo.toml (144 lines)
git-town.toml (4 lines)
justfile (102 lines)
README.md (95 lines)
run.sh (7 lines)
rust-toolchain.toml (2 lines)
```