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
.cursor/
  plans/
    codex_adoption_plan_79b27193.plan.md (208 lines)
    local_cargo_release_management_5bfb9040.plan.md (149 lines)
    next-lovable_migrate_support_eb735df7.plan.md (218 lines)
  rules/
    00-core.mdc (64 lines)
    10-rust-architecture.mdc (62 lines)
    20-plugin-development.mdc (82 lines)
    30-vfs-trait.mdc (54 lines)
    40-concurrency.mdc (60 lines)
    90-ai-fix.mdc (90 lines)
  skills/
    cli-architecture/
      SKILL.md (137 lines)
    plugin-system/
      SKILL.md (151 lines)
    sandbox/
      SKILL.md (209 lines)
    ssg-migrator/
      SKILL.md (158 lines)
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
        mod.rs (178 lines)
        tool.rs (64 lines)
      aisdk_client.rs (654 lines)
      client.rs (422 lines)
      error.rs (60 lines)
      lib.rs (28 lines)
      parse.rs (138 lines)
      types.rs (170 lines)
    Cargo.toml (22 lines)
  api/
    src/
      endpoints/
        aliases.rs (59 lines)
        auth.rs (230 lines)
        deployments.rs (119 lines)
        domains.rs (64 lines)
        gen.rs (35 lines)
        mod.rs (19 lines)
        plugins.rs (28 lines)
        projects.rs (60 lines)
        teams.rs (154 lines)
        users.rs (20 lines)
      http/
        error_mapper.rs (44 lines)
        response_handler.rs (172 lines)
      middleware/
        auth.rs (154 lines)
        mod.rs (4 lines)
        retry.rs (105 lines)
        team.rs (36 lines)
        tracing.rs (75 lines)
      model/
        types.rs (1 lines)
      client.rs (558 lines)
      error.rs (77 lines)
      lib.rs (63 lines)
      models.rs (365 lines)
    API_ENDPOINT_RULES.md (447 lines)
    Cargo.toml (22 lines)
    README.md (111 lines)
  app/
    src/
      commands/
        aliases/
          ls.rs (73 lines)
          mod.rs (74 lines)
          rm.rs (56 lines)
        domains/
          ls.rs (63 lines)
          mod.rs (37 lines)
          rm.rs (45 lines)
        projects/
          add.rs (57 lines)
          ls.rs (74 lines)
          mod.rs (107 lines)
          rm.rs (58 lines)
        promote/
          mod.rs (130 lines)
          status.rs (248 lines)
        rollback/
          mod.rs (117 lines)
          status.rs (248 lines)
        skills/
          add.rs (206 lines)
          audit.rs (537 lines)
          list.rs (104 lines)
          mod.rs (96 lines)
          remove.rs (86 lines)
          validate.rs (330 lines)
        teams/
          add.rs (200 lines)
          invite.rs (38 lines)
          list.rs (86 lines)
          mod.rs (105 lines)
          rm.rs (55 lines)
          switch.rs (51 lines)
        build.rs (386 lines)
        check.rs (244 lines)
        deploy.rs (713 lines)
        deployment_utils.rs (106 lines)
        dev_server.rs (85 lines)
        dev.rs (285 lines)
        external.rs (92 lines)
        gen.rs (88 lines)
        init.rs (143 lines)
        link.rs (36 lines)
        list.rs (31 lines)
        login.rs (42 lines)
        logout.rs (27 lines)
        ls.rs (65 lines)
        mcp_server.rs (11 lines)
        migrate.rs (11 lines)
        mod.rs (79 lines)
        plan.rs (17 lines)
        plugin.rs (109 lines)
        preview.rs (198 lines)
        recipe_validate.rs (27 lines)
        remove.rs (267 lines)
        run.rs (55 lines)
        self_upgrade_stub.rs (52 lines)
        self_upgrade.rs (183 lines)
        site.rs (425 lines)
        switch.rs (51 lines)
        unlink.rs (29 lines)
        version.rs (16 lines)
      detectors/
        detect.rs (678 lines)
        filesystem.rs (62 lines)
        mod.rs (5 lines)
        README.md (133 lines)
      project/
        config.rs (94 lines)
        connect_git.rs (59 lines)
        edit_project_settings.rs (202 lines)
        humanize_path.rs (24 lines)
        input_project.rs (198 lines)
        input_root_directory.rs (79 lines)
        select_org.rs (79 lines)
      recipe/
        deploy/
          cleanup.rs (42 lines)
          clear_paths.rs (44 lines)
          copy_dirs.rs (120 lines)
          env.rs (36 lines)
          info.rs (20 lines)
          lock.rs (89 lines)
          mod.rs (12 lines)
          release.rs (66 lines)
          setup.rs (41 lines)
          shared.rs (72 lines)
          symlink.rs (50 lines)
          update_code.rs (52 lines)
          writable.rs (43 lines)
        tools/
          ddev/
            common.rs (110 lines)
            install.rs (155 lines)
            mkcert.rs (8 lines)
            mod.rs (57 lines)
            uninstall.rs (127 lines)
            verify.rs (24 lines)
          docker/
            common.rs (79 lines)
            install.rs (309 lines)
            mod.rs (33 lines)
            verify.rs (30 lines)
          mise.rs (214 lines)
          mod.rs (23 lines)
        common.rs (50 lines)
        laravel.rs (288 lines)
        mod.rs (85 lines)
        test.rs (12 lines)
        vercel.rs (34 lines)
      services/
        mod.rs (4 lines)
        template_error.rs (36 lines)
        template.rs (677 lines)
      systems/
        analyze.rs (2 lines)
        bootstrap.rs (61 lines)
        execute.rs (2 lines)
        mod.rs (5 lines)
        startup.rs (34 lines)
        version_check.rs (179 lines)
      templates/
        mod.rs (90 lines)
      utils/
        ext.rs (0 lines)
        fs.rs (51 lines)
        json.rs (1 lines)
        mod.rs (3 lines)
      wasm/
        host_functions/
          context.rs (186 lines)
          execution.rs (336 lines)
          filesystem.rs (90 lines)
          helpers.rs (44 lines)
          host.rs (120 lines)
          interaction.rs (145 lines)
          mod.rs (17 lines)
          plugin_ast.rs (154 lines)
          plugin_check.rs (95 lines)
          plugin_fs.rs (438 lines)
          plugin_git.rs (127 lines)
          plugin_sandbox.rs (159 lines)
          plugin_site.rs (34 lines)
          registry.rs (238 lines)
          stubs.rs (40 lines)
          utils.rs (253 lines)
        mod.rs (6 lines)
        plugin.rs (794 lines)
        types.rs (592 lines)
      app_error.rs (79 lines)
      app.rs (374 lines)
      appz-cli.code-workspace (11 lines)
      auth.rs (414 lines)
      context.rs (47 lines)
      host.rs (159 lines)
      http.rs (267 lines)
      importer.rs (1049 lines)
      lib.rs (26 lines)
      log.rs (7 lines)
      project.rs (792 lines)
      sandbox_helpers.rs (37 lines)
      session.rs (345 lines)
      shell.rs (602 lines)
      ssh.rs (159 lines)
      tunnel.rs (332 lines)
    Cargo.toml (82 lines)
  appz_pdk/
    src/
      lib.rs (49 lines)
      macros.rs (156 lines)
      plugin_macros.rs (142 lines)
      prelude.rs (9 lines)
      security.rs (61 lines)
      types.rs (654 lines)
    Cargo.toml (22 lines)
    README.md (155 lines)
  checker/
    src/
      ai_fixer/
        agents.rs (488 lines)
        context.rs (499 lines)
        llm.rs (24 lines)
        mod.rs (576 lines)
        patch.rs (583 lines)
        safety.rs (188 lines)
      providers/
        biome.rs (233 lines)
        clippy.rs (218 lines)
        helpers.rs (87 lines)
        mod.rs (13 lines)
        phpstan.rs (166 lines)
        ruff.rs (241 lines)
        secrets.rs (177 lines)
        stylelint.rs (176 lines)
        typescript.rs (152 lines)
      cache.rs (147 lines)
      config.rs (271 lines)
      detect.rs (219 lines)
      error.rs (143 lines)
      fixer.rs (205 lines)
      git.rs (87 lines)
      init.rs (221 lines)
      lib.rs (77 lines)
      output.rs (227 lines)
      provider.rs (192 lines)
      runner.rs (483 lines)
    Cargo.toml (29 lines)
  cli/
    src/
      main.rs (190 lines)
    Cargo.toml (40 lines)
  command/
    src/
      error.rs (51 lines)
      exec.rs (473 lines)
      lib.rs (8 lines)
      shell.rs (127 lines)
    Cargo.toml (10 lines)
  common/
    src/
      consts.rs (107 lines)
      env.rs (83 lines)
      hardening.rs (94 lines)
      head_tail_buffer.rs (219 lines)
      id.rs (122 lines)
      lib.rs (21 lines)
      path.rs (208 lines)
      types.rs (21 lines)
      user_config.rs (237 lines)
    Cargo.toml (17 lines)
    README.md (35 lines)
  deployer/
    src/
      providers/
        aws_s3.rs (189 lines)
        azure_static.rs (239 lines)
        cloudflare_pages.rs (255 lines)
        firebase.rs (257 lines)
        fly.rs (207 lines)
        github_pages.rs (227 lines)
        helpers.rs (67 lines)
        mod.rs (17 lines)
        netlify.rs (285 lines)
        render.rs (157 lines)
        surge.rs (188 lines)
        vercel.rs (330 lines)
      config.rs (431 lines)
      detect.rs (251 lines)
      error.rs (146 lines)
      lib.rs (95 lines)
      output.rs (155 lines)
      provider.rs (243 lines)
      ui.rs (88 lines)
    Cargo.toml (21 lines)
  dev-server/
    src/
      handlers/
        form_data.rs (155 lines)
        mod.rs (7 lines)
        static_files.rs (274 lines)
        websocket.rs (141 lines)
      config.rs (63 lines)
      error.rs (41 lines)
      lib.rs (11 lines)
      server.rs (168 lines)
      watcher.rs (130 lines)
    Cargo.toml (23 lines)
    README.md (153 lines)
  env_var/
    src/
      env_scanner.rs (45 lines)
      env_substitutor.rs (169 lines)
      global_bag.rs (153 lines)
      lib.rs (7 lines)
    Cargo.toml (11 lines)
  frameworks/
    data/
      frameworks.json (1088 lines)
      php-frameworks.json (250 lines)
    src/
      frameworks.rs (54 lines)
      lib.rs (5 lines)
      types.rs (158 lines)
    .gitignore (3 lines)
    build.rs (350 lines)
    Cargo.toml (15 lines)
    README.md (119 lines)
  init/
    src/
      providers/
        framework.rs (130 lines)
        git.rs (102 lines)
        local.rs (61 lines)
        mod.rs (7 lines)
        npm.rs (50 lines)
        remote_archive.rs (49 lines)
      sources/
        download.rs (81 lines)
        git.rs (324 lines)
        mod.rs (6 lines)
        npm.rs (119 lines)
        remote_archive.rs (54 lines)
      config.rs (107 lines)
      detect.rs (181 lines)
      error.rs (92 lines)
      lib.rs (52 lines)
      output.rs (16 lines)
      provider.rs (63 lines)
      run.rs (146 lines)
      ui.rs (65 lines)
    Cargo.toml (25 lines)
  mcp-server/
    src/
      auth.rs (48 lines)
      lib.rs (10 lines)
      main.rs (13 lines)
      tools.rs (493 lines)
    Cargo.toml (28 lines)
    README.md (71 lines)
  plugin-build/
    src/
      lib.rs (464 lines)
      main.rs (136 lines)
      wasm_header.rs (79 lines)
    Cargo.toml (25 lines)
  plugin-manager/
    src/
      cache.rs (104 lines)
      downloader.rs (76 lines)
      entitlements.rs (140 lines)
      error.rs (127 lines)
      lib.rs (282 lines)
      manifest.rs (170 lines)
      security.rs (401 lines)
      update_check.rs (106 lines)
    Cargo.toml (42 lines)
  plugins/
    check/
      src/
        lib.rs (225 lines)
      Cargo.toml (15 lines)
    site/
      src/
        lib.rs (282 lines)
      Cargo.toml (15 lines)
    ssg-migrator/
      src/
        lib.rs (666 lines)
        vfs_wasm.rs (255 lines)
      Cargo.toml (21 lines)
    Cargo.toml (2 lines)
  sandbox/
    src/
      local/
        mod.rs (429 lines)
      config.rs (206 lines)
      error.rs (197 lines)
      json_ops.rs (203 lines)
      lib.rs (187 lines)
      mise.rs (478 lines)
      provider.rs (337 lines)
      scoped_fs.rs (1070 lines)
      toml_ops.rs (131 lines)
    Cargo.toml (27 lines)
  site-builder/
    src/
      firecrawl/
        mod.rs (206 lines)
        types.rs (211 lines)
      pipeline/
        analyze.rs (297 lines)
        assets.rs (128 lines)
        build.rs (57 lines)
        crawl.rs (34 lines)
        generate.rs (544 lines)
        mod.rs (209 lines)
        scaffold.rs (275 lines)
      prompts/
        assemble.rs (270 lines)
        classify.rs (85 lines)
        ia.rs (96 lines)
        mod.rs (6 lines)
        transform.rs (48 lines)
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
        mod.rs (45 lines)
      themes/
        presets/
          corporate.rs (37 lines)
          minimal.rs (37 lines)
          mod.rs (11 lines)
          nonprofit.rs (37 lines)
          startup.rs (37 lines)
        css.rs (126 lines)
        mod.rs (136 lines)
        tailwind.rs (46 lines)
      cache.rs (102 lines)
      config.rs (139 lines)
      error.rs (79 lines)
      lib.rs (13 lines)
    Cargo.toml (25 lines)
  ssg-migrator/
    src/
      generator/
        classifier.rs (206 lines)
        components.rs (158 lines)
        config.rs (122 lines)
        files.rs (65 lines)
        fix_imports.rs (146 lines)
        layout.rs (47 lines)
        mod.rs (108 lines)
        pages.rs (66 lines)
        readme.rs (48 lines)
        regex.rs (41 lines)
        templates.rs (15 lines)
        transform.rs (327 lines)
      nextjs/
        config.rs (168 lines)
        convert.rs (186 lines)
        mod.rs (76 lines)
        pages.rs (89 lines)
        providers.rs (48 lines)
        regex.rs (43 lines)
        templates.rs (95 lines)
        transform.rs (220 lines)
        verify.rs (164 lines)
      sync/
        backward.rs (51 lines)
        forward.rs (61 lines)
        mod.rs (209 lines)
      analyzer.rs (343 lines)
      ast_transformer.rs (252 lines)
      common.rs (77 lines)
      lib.rs (39 lines)
      transformer.rs (160 lines)
      types.rs (61 lines)
      vfs_native.rs (197 lines)
      vfs.rs (59 lines)
    Cargo.toml (36 lines)
  studio/
    src/
      apply.rs (88 lines)
      lib.rs (18 lines)
      main.rs (3 lines)
      parse.rs (88 lines)
      scaffold.rs (142 lines)
    Cargo.toml (13 lines)
  task/
    src/
      context.rs (211 lines)
      deps.rs (344 lines)
      error.rs (35 lines)
      lib.rs (19 lines)
      registry.rs (188 lines)
      runner.rs (586 lines)
      scheduler.rs (336 lines)
      source_tracker.rs (277 lines)
      task_executor.rs (431 lines)
      task.rs (202 lines)
      types.rs (33 lines)
    Cargo.toml (18 lines)
  ui/
    src/
      banner.rs (107 lines)
      empty.rs (51 lines)
      error.rs (85 lines)
      format.rs (231 lines)
      layout.rs (67 lines)
      lib.rs (22 lines)
      list.rs (143 lines)
      pagination.rs (109 lines)
      progress.rs (215 lines)
      prompt.rs (253 lines)
      status.rs (90 lines)
      table.rs (229 lines)
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
        lib.rs (152 lines)
      Cargo.toml (15 lines)
      README.md (23 lines)
  recipes/
    common.yaml (35 lines)
    deploy.yaml (21 lines)
  comprehensive-recipe.yaml (193 lines)
packages/
  appz-mcp-server/
    bin/
      mcp-server.js (47 lines)
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
justfile (100 lines)
README.md (95 lines)
repomix-output.xml (83993 lines)
run.sh (7 lines)
rust-toolchain.toml (2 lines)
```