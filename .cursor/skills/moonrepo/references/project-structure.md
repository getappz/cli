# Directory Structure

```
.cargo/
  config.toml (29 lines)
  nextest.toml (8 lines)
.github/
  ISSUE_TEMPLATE/
    bug_report.md (40 lines)
    feature_request.md (24 lines)
  workflows/
    benchmark.ymldisabled (78 lines)
    docs.yml (19 lines)
    moon.yml (82 lines)
    pr.yml (38 lines)
    release-npm.yml (39 lines)
    release.yml (321 lines)
    rust.yml (164 lines)
  copilot-instructions.md (66 lines)
  FUNDING.yml (1 lines)
.moon/
  tasks/
    bash.yml (7 lines)
    node.yml (101 lines)
  toolchains.yml (21 lines)
  workspace.yml (63 lines)
crates/
  action/
    src/
      action_node.rs (136 lines)
      action.rs (72 lines)
      lib.rs (5 lines)
      operation_list.rs (68 lines)
      operation_meta.rs (58 lines)
      operation.rs (161 lines)
    Cargo.toml (18 lines)
  action-context/
    src/
      lib.rs (84 lines)
    Cargo.toml (17 lines)
  action-graph/
    src/
      action_graph_builder.rs (580 lines)
      action_graph_error.rs (5 lines)
      action_graph.rs (99 lines)
      lib.rs (3 lines)
    tests/
      __fixtures__/
        dep-workspace/
          .moon/
            toolchains.yml (2 lines)
          in/
            moon.yml (1 lines)
          isolated/
            moon.yml (1 lines)
          out/
            moon.yml (1 lines)
          moon.yml (1 lines)
          package.json (3 lines)
        projects/
          .moon/
            toolchains.yml (6 lines)
          bar/
            moon.yml (1 lines)
          baz/
            moon.yml (5 lines)
          foo/
            moon.yml (1 lines)
          qux/
            moon.yml (5 lines)
          package.json (3 lines)
        tasks/
          base/
            moon.yml (7 lines)
          ci/
            moon.yml (45 lines)
          client/
            moon.yml (14 lines)
          common/
            moon.yml (13 lines)
          deps/
            moon.yml (43 lines)
          deps-affected/
            moon.yml (16 lines)
          deps-external/
            moon.yml (3 lines)
          misc/
            moon.yml (4 lines)
          partition/
            moon.yml (11 lines)
          server/
            moon.yml (10 lines)
        tasks-ci-mismatch/
          ci/
            moon.yml (12 lines)
      action_graph_builder_test.rs (537 lines)
      utils.rs (34 lines)
    Cargo.toml (41 lines)
  action-pipeline/
    src/
      reports/
        estimate.rs (80 lines)
        mod.rs (1 lines)
      subscribers/
        cleanup_subscriber.rs (22 lines)
        console_subscriber.rs (23 lines)
        mod.rs (7 lines)
        notifications_subscriber.rs (35 lines)
        remote_subscriber.rs (16 lines)
        reports_subscriber.rs (36 lines)
        telemetry_subscriber.rs (27 lines)
        webhooks_subscriber.rs (21 lines)
      action_pipeline.rs (269 lines)
      action_runner.rs (89 lines)
      event_emitter.rs (47 lines)
      job_context.rs (46 lines)
      job_dispatcher.rs (78 lines)
      job.rs (44 lines)
      lib.rs (8 lines)
    tests/
      __fixtures__/
        pipeline/
          priority/
            moon.yml (39 lines)
      action_pipeline_test.rs (44 lines)
      report_estimate_test.rs (48 lines)
    Cargo.toml (43 lines)
  actions/
    src/
      actions/
        install_dependencies.rs (196 lines)
        mod.rs (7 lines)
        run_task.rs (26 lines)
        setup_environment.rs (76 lines)
        setup_proto.rs (75 lines)
        setup_toolchain.rs (54 lines)
        sync_project.rs (69 lines)
        sync_workspace.rs (70 lines)
      operations/
        mod.rs (3 lines)
        sync_codeowners.rs (56 lines)
        sync_config_schemas.rs (30 lines)
        sync_vcs_hooks.rs (30 lines)
      plugins/
        commands.rs (170 lines)
        mod.rs (2 lines)
        operations.rs (51 lines)
      lib.rs (4 lines)
      utils.rs (96 lines)
    tests/
      __fixtures__/
        projects/
          a/
            moon.yml (4 lines)
          b/
            moon.yml (6 lines)
          c/
            moon.yml (7 lines)
      commands_test.rs (208 lines)
      sync_project_test.rs (86 lines)
      sync_workspace_test.rs (115 lines)
    Cargo.toml (50 lines)
  affected/
    src/
      affected_tracker.rs (237 lines)
      affected.rs (58 lines)
      lib.rs (2 lines)
    tests/
      __fixtures__/
        projects/
          a/
            moon.yml (1 lines)
          b/
            moon.yml (1 lines)
          c/
            moon.yml (1 lines)
          cycle-a/
            moon.yml (1 lines)
          cycle-b/
            moon.yml (1 lines)
          cycle-c/
            moon.yml (1 lines)
          d/
            moon.yml (0 lines)
          e/
            moon.yml (0 lines)
          moon.yml (1 lines)
        tasks/
          base/
            moon.yml (21 lines)
          chain/
            moon.yml (19 lines)
          ci/
            moon.yml (19 lines)
          cycle/
            moon.yml (10 lines)
          dep/
            moon.yml (5 lines)
          dep-test/
            moon.yml (3 lines)
          downstream/
            moon.yml (4 lines)
          parent/
            moon.yml (9 lines)
          project-sources/
            moon.yml (19 lines)
          self/
            moon.yml (6 lines)
          upstream/
            moon.yml (3 lines)
      affected_tracker_test.rs (260 lines)
    Cargo.toml (32 lines)
  api/
    src/
      launchpad.rs (114 lines)
      lib.rs (1 lines)
    Cargo.toml (26 lines)
  app/
    src/
      commands/
        debug/
          config.rs (13 lines)
          mod.rs (6 lines)
          vcs.rs (24 lines)
        docker/
          docker_error.rs (6 lines)
          file.rs (114 lines)
          mod.rs (13 lines)
          prune.rs (108 lines)
          scaffold.rs (192 lines)
          setup.rs (25 lines)
        extension/
          add.rs (60 lines)
          info.rs (43 lines)
          mod.rs (6 lines)
        graph/
          dto.rs (3 lines)
          html.tera (22 lines)
          mod.rs (2 lines)
          utils.rs (113 lines)
        migrate/
          mod.rs (5 lines)
          v2.rs (301 lines)
        query/
          affected.rs (26 lines)
          changed_files.rs (21 lines)
          mod.rs (8 lines)
          projects.rs (35 lines)
          tasks.rs (42 lines)
        syncs/
          codeowners.rs (26 lines)
          config_schemas.rs (18 lines)
          mod.rs (11 lines)
          projects.rs (27 lines)
          vcs_hooks.rs (31 lines)
        toolchain/
          add.rs (72 lines)
          info.rs (47 lines)
          mod.rs (6 lines)
        action_graph.rs (36 lines)
        bin.rs (25 lines)
        check.rs (54 lines)
        ci.rs (32 lines)
        clean.rs (17 lines)
        completions.rs (28 lines)
        exec.rs (281 lines)
        ext.rs (18 lines)
        generate.rs (266 lines)
        hash.rs (78 lines)
        init.rs (71 lines)
        mcp.rs (30 lines)
        mod.rs (33 lines)
        project_graph.rs (29 lines)
        project.rs (40 lines)
        projects.rs (28 lines)
        run.rs (13 lines)
        setup.rs (69 lines)
        sync.rs (26 lines)
        task_graph.rs (29 lines)
        task.rs (84 lines)
        tasks.rs (35 lines)
        teardown.rs (26 lines)
        template.rs (58 lines)
        templates.rs (38 lines)
        upgrade.rs (82 lines)
      components/
        api_list.rs (4 lines)
        config_settings.rs (9 lines)
        mod.rs (2 lines)
      queries/
        changed_files.rs (119 lines)
        mod.rs (17 lines)
        projects.rs (74 lines)
        tasks.rs (67 lines)
      systems/
        analyze.rs (26 lines)
        bootstrap.rs (79 lines)
        execute.rs (21 lines)
        mod.rs (4 lines)
        startup.rs (116 lines)
      app_error.rs (5 lines)
      app_options.rs (57 lines)
      app.rs (78 lines)
      helpers.rs (96 lines)
      lib.rs (10 lines)
      prompts.rs (157 lines)
      session.rs (211 lines)
    Cargo.toml (98 lines)
  app-context/
    src/
      app_context.rs (20 lines)
      lib.rs (3 lines)
    Cargo.toml (18 lines)
  app-macros/
    src/
      lib.rs (47 lines)
    Cargo.toml (17 lines)
  cache/
    src/
      cache_engine.rs (122 lines)
      hash_engine.rs (59 lines)
      lib.rs (17 lines)
      state_engine.rs (68 lines)
    tests/
      cache_engine_test.rs (45 lines)
      hash_engine_test.rs (26 lines)
    Cargo.toml (27 lines)
  cache-item/
    src/
      cache_item.rs (26 lines)
      cache_mode.rs (33 lines)
      lib.rs (8 lines)
    tests/
      __fixtures__/
        item/
          data.json (3 lines)
      cache_item_test.rs (80 lines)
    Cargo.toml (23 lines)
  cli/
    src/
      lookup.rs (121 lines)
      main_exec.rs (10 lines)
      main.rs (8 lines)
      shared.rs (108 lines)
    tests/
      __fixtures__/
        docker/
          .moon/
            toolchains.yml (1 lines)
            workspace.yml (2 lines)
          dep/
            file.txt (0 lines)
            moon.yml (2 lines)
            tc.cfg (3 lines)
          prune/
            vendor/
              package.cfg (0 lines)
            file.txt (0 lines)
            moon.yml (2 lines)
            tc.cfg (3 lines)
          scaffold/
            file.txt (0 lines)
            moon.yml (4 lines)
            tc.cfg (3 lines)
          tc.lock (1 lines)
          tc.root.cfg (4 lines)
        dockerfile/
          has-tasks/
            moon.yml (3 lines)
          no-tasks/
            moon.yml (0 lines)
          with-config/
            moon.yml (9 lines)
        extensions/
          .moon/
            extensions.yml (3 lines)
        generator/
          templates/
            configs/
              file.json (7 lines)
              file.yaml (4 lines)
              template.yml (4 lines)
            dest/
              file.txt (0 lines)
              template.yml (9 lines)
            extends/
              base.txt (0 lines)
              template.yml (9 lines)
              two.txt (1 lines)
              vars.txt (2 lines)
            extends-one/
              one.txt (1 lines)
              template.yml (9 lines)
            extends-two/
              template.yml (8 lines)
              two.txt (1 lines)
            extensions/
              file.ts.tera (1 lines)
              file.tsx.twig (1 lines)
              template.yml (9 lines)
            frontmatter/
              component.tsx (0 lines)
              forced.txt (5 lines)
              skipped.txt (5 lines)
              template.yml (4 lines)
              to.txt (5 lines)
            standard/
              folder/
                nested-file.ts (0 lines)
              file.ts (0 lines)
              file.txt (2 lines)
              other.raw.txt (2 lines)
              template.yml (4 lines)
            vars/
              control.txt (18 lines)
              expressions.txt (34 lines)
              file-[stringNotEmpty]-[number].txt (0 lines)
              filters.txt (29 lines)
              partial.txt (1 lines)
              template.yml (134 lines)
            vars-collections/
              print.txt (2 lines)
              template.yml (15 lines)
        pipeline/
          check-a/
            moon.yml (11 lines)
          check-b/
            moon.yml (3 lines)
          shared/
            moon.yml (16 lines)
          unix/
            affectedFiles.sh (5 lines)
            args.sh (6 lines)
            cwd.sh (4 lines)
            envVars.sh (6 lines)
            envVarsMoon.sh (6 lines)
            exitNonZero.sh (9 lines)
            exitZero.sh (9 lines)
            moon.yml (165 lines)
            outputs.sh (6 lines)
            standard.sh (5 lines)
          windows/
            affectedFiles.ps1 (5 lines)
            args.ps1 (9 lines)
            cwd.ps1 (4 lines)
            envVars.ps1 (6 lines)
            envVarsMoon.ps1 (7 lines)
            exitNonZero.ps1 (10 lines)
            exitZero.ps1 (10 lines)
            moon.yml (192 lines)
            outputs.ps1 (6 lines)
            standard.ps1 (6 lines)
        projects/
          advanced/
            moon.yml (9 lines)
          basic/
            moon.yml (15 lines)
          dep-bar/
            empty (0 lines)
          dep-baz/
            empty (0 lines)
          dep-foo/
            moon.yml (8 lines)
          empty-config/
            moon.yml (1 lines)
          metadata/
            moon.yml (19 lines)
          no-config/
            empty (0 lines)
          tasks/
            moon.yml (15 lines)
          toolchains/
            moon.yml (9 lines)
          moon.yml (1 lines)
        tasks/
          basic/
            moon.yml (12 lines)
            package.json (3 lines)
          build-a/
            moon.yml (14 lines)
            package.json (3 lines)
          build-b/
            moon.yml (5 lines)
            package.json (3 lines)
          build-c/
            moon.yml (6 lines)
            package.json (3 lines)
          chain/
            moon.yml (20 lines)
            package.json (3 lines)
          no-tasks/
            moon.yml (3 lines)
            package.json (3 lines)
          node/
            moon.yml (16 lines)
            package.json (3 lines)
          package.json (5 lines)
        tasks-cycle/
          cycle/
            moon.yml (12 lines)
      action_graph_test.rs (59 lines)
      check_test.rs (32 lines)
      docker_file_test.rs (67 lines)
      docker_prune_test.rs (35 lines)
      docker_scaffold_test.rs (78 lines)
      docker_setup_test.rs (10 lines)
      exec_test.rs (308 lines)
      ext_test.rs (70 lines)
      extension_add_test.rs (38 lines)
      extension_info_test.rs (22 lines)
      generate_test.rs (316 lines)
      hash_test.rs (46 lines)
      init_test.rs (54 lines)
      misc_test.rs (43 lines)
      project_graph_test.rs (40 lines)
      project_test.rs (65 lines)
      projects_test.rs (25 lines)
      query_affected_test.rs (33 lines)
      query_changed_files_test.rs (31 lines)
      query_projects_test.rs (123 lines)
      query_tasks_test.rs (106 lines)
      run_legacy_test.rs (761 lines)
      run_webhooks_test.rs (52 lines)
      setup_teardown_test.rs (36 lines)
      sync_test.rs (77 lines)
      task_graph_test.rs (40 lines)
      task_test.rs (38 lines)
      tasks_test.rs (33 lines)
      template_test.rs (27 lines)
      templates_test.rs (30 lines)
      toolchain_add_test.rs (38 lines)
      toolchain_info_test.rs (22 lines)
      utils.rs (43 lines)
    Cargo.toml (71 lines)
  codegen/
    src/
      asset_file.rs (16 lines)
      codegen_error.rs (6 lines)
      codegen.rs (227 lines)
      filters.rs (72 lines)
      funcs.rs (10 lines)
      lib.rs (9 lines)
      template_file.rs (81 lines)
      template.rs (185 lines)
    tests/
      __fixtures__/
        dupes/
          templates/
            by-id/
              template.yml (3 lines)
            folder-name/
              template.yml (2 lines)
        generator/
          templates/
            extends/
              b.txt (1 lines)
              base.txt (0 lines)
              template.yml (10 lines)
            extends-from-a/
              a.txt (1 lines)
              template.yml (5 lines)
            extends-from-b/
              b.txt (1 lines)
              template.yml (6 lines)
            extends-from-c/
              c.txt (1 lines)
              template.yml (5 lines)
            extends-unknown/
              template.yml (3 lines)
            one/
              file.txt (0 lines)
              template.yml (2 lines)
            two/
              file.txt (0 lines)
              template.yml (2 lines)
        include/
          templates/
            base/
              include.txt (7 lines)
              inheritance.txt (17 lines)
              local.txt (1 lines)
              template.yml (3 lines)
            extended/
              full.txt (1 lines)
              macros.txt (6 lines)
              template.yml (3 lines)
              wrapper.txt (17 lines)
            partials/
              partials/
                part.txt (1 lines)
              template.yml (2 lines)
        template/
          folder/
            nested-file.ts (0 lines)
          file.raw.txt (2 lines)
          file.ts (0 lines)
          file.txt (2 lines)
          partial-file.ts (0 lines)
          template.yml (4 lines)
      codegen_test.rs (118 lines)
      template_file_test.rs (74 lines)
      template_test.rs (157 lines)
    Cargo.toml (41 lines)
  codeowners/
    src/
      codeowners_fingerprint.rs (9 lines)
      codeowners_generator.rs (88 lines)
      lib.rs (2 lines)
    tests/
      __fixtures__/
        custom-groups/
          moon.yml (5 lines)
        list-paths/
          moon.yml (6 lines)
        map-paths/
          moon.yml (6 lines)
        no-paths/
          moon.yml (1 lines)
        workspace/
          workspace.yml (5 lines)
      codeowners_generator_test.rs (46 lines)
    Cargo.toml (24 lines)
  common/
    src/
      env.rs (48 lines)
      id.rs (15 lines)
      lib.rs (4 lines)
      macros.rs (3 lines)
      path.rs (120 lines)
    tests/
      id_test.rs (24 lines)
    Cargo.toml (24 lines)
  config/
    src/
      formats/
        hcl.rs (71 lines)
        mod.rs (1 lines)
      project/
        dep_config.rs (38 lines)
        docker_config.rs (19 lines)
        language.rs (54 lines)
        mod.rs (5 lines)
        overrides_config.rs (69 lines)
        owners_config.rs (72 lines)
      shapes/
        input.rs (245 lines)
        mod.rs (47 lines)
        output.rs (126 lines)
        poly.rs (59 lines)
        portable_path.rs (139 lines)
      template/
        frontmatter.rs (34 lines)
        mod.rs (2 lines)
        template_locator.rs (69 lines)
      toolchain/
        bin_config.rs (26 lines)
        mod.rs (3 lines)
        moon_config.rs (11 lines)
        proto_config.rs (16 lines)
      workspace/
        codeowners_config.rs (39 lines)
        constraints_config.rs (20 lines)
        docker_config.rs (116 lines)
        experiments_config.rs (6 lines)
        generator_config.rs (16 lines)
        hasher_config.rs (56 lines)
        mod.rs (10 lines)
        notifier_config.rs (37 lines)
        pipeline_config.rs (72 lines)
        remote_config.rs (178 lines)
        vcs_config.rs (60 lines)
      config_cache.rs (60 lines)
      config_finder.rs (81 lines)
      config_loader.rs (195 lines)
      extensions_config.rs (87 lines)
      inherited_tasks_config.rs (224 lines)
      inherited_tasks_manager.rs (59 lines)
      lib.rs (31 lines)
      macros.rs (13 lines)
      patterns.rs (33 lines)
      project_config.rs (136 lines)
      task_config.rs (195 lines)
      task_options_config.rs (285 lines)
      template_config.rs (174 lines)
      test_utils.rs (22 lines)
      toolchains_config.rs (162 lines)
      workspace_config.rs (148 lines)
    tests/
      __fixtures__/
        extends/
          tasks/
            global-0.yml (5 lines)
            global-1.yml (11 lines)
            global-2.yml (11 lines)
          toolchain/
            base-0.yml (3 lines)
            base-1.yml (7 lines)
            base-2.yml (5 lines)
            typescript-0.yml (2 lines)
            typescript-1.yml (4 lines)
            typescript-2.yml (6 lines)
          workspace/
            base-0.yml (2 lines)
            base-1.yml (7 lines)
            base-2.yml (4 lines)
        hcl/
          .moon/
            extensions.hcl (4 lines)
            tasks.hcl (107 lines)
            toolchains.hcl (20 lines)
            workspace.hcl (76 lines)
          moon.hcl (76 lines)
          task.hcl (51 lines)
          template.hcl (39 lines)
        inheritance/
          files/
            tasks/
              all.yml (3 lines)
              bun.yml (6 lines)
              deno.yml (6 lines)
              javascript-library.yml (7 lines)
              javascript-tool.yml (7 lines)
              javascript.yml (6 lines)
              kotlin.yml (6 lines)
              node-application.yml (7 lines)
              node-library.yml (7 lines)
              node.yml (6 lines)
              python.yml (6 lines)
              rust.yml (6 lines)
              tag-camelCase.yml (6 lines)
              tag-dot.case.yml (6 lines)
              tag-kebab-case.yml (6 lines)
              tag-normal.yml (6 lines)
              typescript.yml (6 lines)
          nested/
            tasks/
              dotnet/
                dotnet-application.yml (4 lines)
                dotnet.yml (4 lines)
              node/
                node-library.yml (4 lines)
                node.yml (4 lines)
              all.yml (4 lines)
        pkl/
          .moon/
            extensions.pkl (4 lines)
            tasks.pkl (104 lines)
            toolchains.pkl (20 lines)
            workspace.pkl (74 lines)
          moon.pkl (78 lines)
          task.pkl (51 lines)
          template.pkl (37 lines)
        toml/
          .moon/
            extensions.toml (3 lines)
            tasks.toml (80 lines)
            toolchains.toml (17 lines)
            workspace.toml (58 lines)
          moon.toml (54 lines)
          task.toml (34 lines)
          template.toml (28 lines)
      extensions_config_test.rs (41 lines)
      inherited_by_test.rs (183 lines)
      inherited_tasks_config_test.rs (214 lines)
      input_shape_test.rs (310 lines)
      output_shape_test.rs (156 lines)
      project_config_test.rs (168 lines)
      task_config_test.rs (216 lines)
      template_config_test.rs (81 lines)
      template_frontmatter_test.rs (35 lines)
      toolchains_config_test.rs (113 lines)
      utils.rs (77 lines)
      workspace_config_test.rs (212 lines)
    Cargo.toml (63 lines)
  config-schema/
    src/
      json_schemas.rs (83 lines)
      lib.rs (5 lines)
      main.rs (21 lines)
      typescript_types.rs (67 lines)
    Cargo.toml (22 lines)
  console/
    src/
      lib.rs (31 lines)
      reporter.rs (308 lines)
      theme.rs (7 lines)
    Cargo.toml (24 lines)
  docker/
    src/
      docker_file.rs (29 lines)
      lib.rs (1 lines)
    templates/
      CustomTemplate.tera (11 lines)
      Dockerfile.tera (67 lines)
    tests/
      dockerfile_test.rs (32 lines)
    Cargo.toml (23 lines)
  env/
    src/
      lib.rs (42 lines)
    Cargo.toml (18 lines)
  env-var/
    src/
      dotenv_error.rs (6 lines)
      dotenv.rs (137 lines)
      env_scanner.rs (26 lines)
      env_substitutor.rs (96 lines)
      global_bag.rs (89 lines)
      lib.rs (37 lines)
    tests/
      dotenv_test.rs (132 lines)
      env_substitutor_test.rs (103 lines)
    Cargo.toml (22 lines)
  extension-plugin/
    src/
      extension_plugin.rs (86 lines)
      extension_registry_actions.rs (39 lines)
      extension_registry.rs (125 lines)
      lib.rs (3 lines)
    Cargo.toml (25 lines)
  feature-flags/
    src/
      lib.rs (16 lines)
    Cargo.toml (15 lines)
  file-group/
    src/
      file_group_error.rs (5 lines)
      file_group.rs (150 lines)
      lib.rs (2 lines)
    tests/
      __fixtures__/
        file-group/
          other/
            file.json (0 lines)
          project/
            dir/
              subdir/
                nested.json (0 lines)
            docs.md (0 lines)
            project.json (0 lines)
          docs.md (0 lines)
          workspace.json (0 lines)
      file_group_test.rs (122 lines)
    Cargo.toml (25 lines)
  graph-utils/
    src/
      graph_context.rs (15 lines)
      graph_formats.rs (48 lines)
      graph_traits.rs (120 lines)
      lib.rs (156 lines)
    Cargo.toml (16 lines)
  hash/
    src/
      hasher.rs (46 lines)
      lib.rs (3 lines)
    tests/
      hasher_test.rs (25 lines)
    Cargo.toml (21 lines)
  mcp/
    src/
      tools/
        action_tools.rs (44 lines)
        codegen_tools.rs (52 lines)
        mod.rs (21 lines)
        project_tools.rs (33 lines)
        task_tools.rs (42 lines)
        vcs_tools.rs (30 lines)
      lib.rs (4 lines)
      mcp.rs (59 lines)
    Cargo.toml (34 lines)
  notifier/
    src/
      lib.rs (2 lines)
      notifications.rs (58 lines)
      webhooks.rs (75 lines)
    build.rs (5 lines)
    Cargo.toml (22 lines)
  pdk/
    src/
      args.rs (21 lines)
      extension.rs (36 lines)
      funcs.rs (67 lines)
      lib.rs (10 lines)
      toolchain.rs (104 lines)
    Cargo.toml (32 lines)
  pdk-api/
    src/
      toolchain/
        mod.rs (3 lines)
        tier1.rs (175 lines)
        tier2.rs (263 lines)
        tier3.rs (43 lines)
      common.rs (258 lines)
      context.rs (119 lines)
      extension.rs (40 lines)
      host.rs (69 lines)
      lib.rs (15 lines)
      macros.rs (12 lines)
      prompts.rs (74 lines)
    Cargo.toml (29 lines)
  pdk-test-utils/
    src/
      extension_wrapper.rs (54 lines)
      host_func_mocker.rs (63 lines)
      lib.rs (4 lines)
      sandbox.rs (134 lines)
      toolchain_wrapper.rs (135 lines)
    Cargo.toml (23 lines)
  plugin/
    src/
      host.rs (105 lines)
      lib.rs (21 lines)
      plugin_error.rs (6 lines)
      plugin_registry.rs (145 lines)
      plugin.rs (20 lines)
    tests/
      plugin_registry_test.rs (50 lines)
    Cargo.toml (38 lines)
  process/
    src/
      command_line.rs (106 lines)
      command.rs (271 lines)
      exec_command.rs (357 lines)
      lib.rs (10 lines)
      main.rs (53 lines)
      output_stream.rs (103 lines)
      output.rs (42 lines)
      process_error.rs (7 lines)
      process_registry.rs (113 lines)
      shared_child.rs (94 lines)
      shell.rs (35 lines)
      signal.rs (76 lines)
    args.ps1 (10 lines)
    args.sh (8 lines)
    Cargo.toml (34 lines)
  process-augment/
    src/
      augmented_command.rs (199 lines)
      lib.rs (1 lines)
    Cargo.toml (20 lines)
  project/
    src/
      lib.rs (4 lines)
      project_error.rs (5 lines)
      project.rs (166 lines)
    Cargo.toml (21 lines)
  project-builder/
    src/
      lib.rs (1 lines)
      project_builder.rs (241 lines)
    tests/
      __fixtures__/
        builder/
          bar/
            moon.yml (9 lines)
          baz/
            moon.yml (10 lines)
            package.json (1 lines)
          foo/
            package.json (1 lines)
          global/
            tasks/
              all.yml (18 lines)
              node.yml (16 lines)
          qux/
            tsconfig.json (1 lines)
        langs/
          bash/
            moon.yml (2 lines)
          batch/
            moon.yml (2 lines)
          bun/
            moon.yml (4 lines)
          bun-config/
            moon.yml (4 lines)
          deno/
            moon.yml (4 lines)
          deno-config/
            deno.json (1 lines)
            moon.yml (4 lines)
          go/
            moon.yml (2 lines)
          go-config/
            go.mod (0 lines)
          js/
            moon.yml (4 lines)
          js-config/
            package.json (1 lines)
          other/
            moon.yml (2 lines)
          php/
            moon.yml (2 lines)
          php-config/
            composer.json (1 lines)
          python/
            moon.yml (2 lines)
          python-config/
            .python-version (0 lines)
          ruby/
            moon.yml (2 lines)
          ruby-config/
            Gemfile (0 lines)
          rust/
            moon.yml (2 lines)
          rust-config/
            Cargo.toml (3 lines)
          ts/
            moon.yml (2 lines)
          ts-config/
            tsconfig.json (1 lines)
          ts-disabled/
            moon.yml (5 lines)
            tsconfig.json (1 lines)
          ts-enabled/
            moon.yml (2 lines)
      project_builder_test.rs (215 lines)
    Cargo.toml (30 lines)
  project-constraints/
    src/
      lib.rs (71 lines)
    tests/
      constraints_test.rs (183 lines)
    Cargo.toml (19 lines)
  project-expander/
    src/
      expander_context.rs (10 lines)
      lib.rs (2 lines)
      project_expander.rs (33 lines)
    Cargo.toml (25 lines)
  project-graph/
    src/
      lib.rs (2 lines)
      project_graph_error.rs (6 lines)
      project_graph.rs (220 lines)
    tests/
      __fixtures__/
        aliases/
          alias-one/
            package.json (3 lines)
          alias-same-id/
            package.json (3 lines)
          alias-three/
            package.json (3 lines)
          alias-two/
            package.json (3 lines)
          dupes-depends-on/
            moon.yml (5 lines)
          dupes-task-deps/
            moon.yml (5 lines)
          explicit/
            moon.yml (5 lines)
          explicit-and-implicit/
            moon.yml (3 lines)
            package.json (6 lines)
          implicit/
            moon.yml (1 lines)
            package.json (9 lines)
          multiple/
            Cargo.toml (2 lines)
            package.json (3 lines)
          tasks/
            moon.yml (6 lines)
          package.json (3 lines)
        aliases-conflict/
          one/
            package.json (3 lines)
          two/
            package.json (3 lines)
        aliases-conflict-ids/
          one/
            package.json (3 lines)
          two/
            moon.yml (0 lines)
        boundaries/
          overlapping-outputs/
            a/
              moon.yml (7 lines)
        custom-id/
          bar/
            moon.yml (5 lines)
          baz/
            moon.yml (5 lines)
          foo/
            moon.yml (6 lines)
        custom-id-conflict/
          foo/
            moon.yml (0 lines)
          foo-other/
            moon.yml (1 lines)
        custom-id-old-ref/
          bar/
            moon.yml (5 lines)
          foo/
            moon.yml (5 lines)
        cycle/
          a/
            moon.yml (2 lines)
          b/
            moon.yml (2 lines)
          c/
            moon.yml (2 lines)
        dependencies/
          a/
            moon.yml (3 lines)
          b/
            moon.yml (2 lines)
          c/
            moon.yml (1 lines)
          d/
            moon.yml (6 lines)
        dependency-types/
          a/
            moon.yml (1 lines)
          b/
            moon.yml (3 lines)
          c/
            moon.yml (3 lines)
          from-root-task-deps/
            moon.yml (4 lines)
          from-task-deps/
            moon.yml (7 lines)
          no-depends-on/
            moon.yml (1 lines)
          self-task-deps/
            moon.yml (6 lines)
          some-depends-on/
            moon.yml (1 lines)
          moon.yml (3 lines)
        dupe-folder-conflict/
          .moon/
            workspace.yml (2 lines)
          one/
            id/
              moon.yml (0 lines)
          two/
            id/
              moon.yml (0 lines)
        dupe-folder-ids/
          .moon/
            workspace.yml (2 lines)
          one/
            id/
              moon.yml (1 lines)
          two/
            id/
              moon.yml (1 lines)
        expansion/
          base/
            moon.yml (1 lines)
          project/
            moon.yml (10 lines)
          tag-one/
            moon.yml (5 lines)
          tag-three/
            moon.yml (5 lines)
          tag-two/
            moon.yml (1 lines)
          tasks/
            moon.yml (27 lines)
        id-formats/
          four/
            five/
              moon.yml (0 lines)
          one/
            two/
              three/
                moon.yml (0 lines)
              moon.yml (0 lines)
            moon.yml (0 lines)
        ids/
          camelCase/
            moon.yml (1 lines)
          Capital/
            moon.yml (1 lines)
          kebab-case/
            moon.yml (1 lines)
          PascalCase/
            moon.yml (1 lines)
          snake_case/
            moon.yml (1 lines)
          With_nums-123/
            moon.yml (1 lines)
        inheritance/
          file-groups/
            .moon/
              tasks/
                all.yml (5 lines)
            project/
              moon.yml (5 lines)
          implicits/
            .moon/
              tasks/
                all.yml (5 lines)
            base/
              moon.yml (2 lines)
            project/
              moon.yml (7 lines)
          scoped/
            .moon/
              tasks/
                all.yml (3 lines)
                javascript.yml (6 lines)
                kotlin.yml (6 lines)
                node-library.yml (7 lines)
                node.yml (6 lines)
                system.yml (6 lines)
                typescript.yml (6 lines)
            bun/
              moon.yml (7 lines)
              package.json (1 lines)
            bun-with-ts/
              moon.yml (9 lines)
              package.json (1 lines)
            deno/
              moon.yml (7 lines)
              package.json (1 lines)
            kotlin-app/
              moon.yml (6 lines)
            node/
              moon.yml (7 lines)
              package.json (1 lines)
            node-library/
              moon.yml (9 lines)
              package.json (1 lines)
            ruby-tool/
              moon.yml (5 lines)
            system-library/
              moon.yml (9 lines)
          tagged/
            .moon/
              tasks/
                tag-armor.yml (6 lines)
                tag-magic.yml (6 lines)
                tag-weapons.yml (6 lines)
            mage/
              moon.yml (5 lines)
            priest/
              moon.yml (5 lines)
            warrior/
              moon.yml (5 lines)
        layer-constraints/
          app/
            moon.yml (1 lines)
          app-other/
            moon.yml (1 lines)
          library/
            moon.yml (1 lines)
          library-other/
            moon.yml (1 lines)
          tool/
            moon.yml (1 lines)
          tool-other/
            moon.yml (1 lines)
          unknown/
            moon.yml (1 lines)
        locate-configs/
          a/
            moon.yml (1 lines)
          b/
            empty (0 lines)
          c/
            moon.yml (1 lines)
          d/
            empty (0 lines)
        query/
          a/
            moon.yml (14 lines)
          b/
            moon.yml (8 lines)
          c/
            moon.yml (8 lines)
          d/
            moon.yml (11 lines)
        tag-constraints/
          a/
            moon.yml (1 lines)
          b/
            moon.yml (1 lines)
          c/
            moon.yml (1 lines)
          package.json (5 lines)
      project_graph_test.rs (564 lines)
    Cargo.toml (36 lines)
  query/
    src/
      builder.rs (86 lines)
      lib.rs (5 lines)
      mql.pest (36 lines)
      parser.rs (60 lines)
      query_error.rs (5 lines)
    tests/
      builder_test.rs (144 lines)
      parser_test.rs (52 lines)
    Cargo.toml (22 lines)
  remote/
    src/
      action_state.rs (82 lines)
      blob.rs (52 lines)
      fs_digest.rs (135 lines)
      grpc_remote_client.rs (274 lines)
      grpc_services.rs (71 lines)
      grpc_tls.rs (46 lines)
      http_remote_client.rs (209 lines)
      http_tls.rs (39 lines)
      lib.rs (19 lines)
      remote_client.rs (46 lines)
      remote_error.rs (8 lines)
      remote_service.rs (284 lines)
    tests/
      __fixtures__/
        certs/
          ca.pem (28 lines)
          client.pem (19 lines)
          README.md (1 lines)
          server.pem (27 lines)
        certs-local/
          ca.crt (29 lines)
          client.crt (29 lines)
          client.csr (26 lines)
          README.md (1 lines)
          server.crt (29 lines)
          server.csr (26 lines)
    Cargo.toml (45 lines)
  task/
    src/
      lib.rs (3 lines)
      task_arg.rs (57 lines)
      task_options.rs (8 lines)
      task.rs (214 lines)
    tests/
      __fixtures__/
        files/
          a.js (0 lines)
          b.js (0 lines)
          c.jsx (0 lines)
          d.rs (0 lines)
      task_test.rs (21 lines)
    Cargo.toml (25 lines)
  task-builder/
    src/
      lib.rs (3 lines)
      task_deps_builder.rs (111 lines)
      tasks_builder_error.rs (6 lines)
      tasks_builder.rs (515 lines)
    tests/
      __fixtures__/
        builder/
          args-glob/
            moon.yml (18 lines)
          commands/
            moon.yml (29 lines)
          dep-a/
            moon.yml (3 lines)
          dep-b/
            moon.yml (3 lines)
          dep-c/
            moon.yml (3 lines)
          env/
            moon.yml (12 lines)
          env-substitute/
            moon.yml (17 lines)
          extends/
            moon.yml (56 lines)
          extends-interweave/
            moon.yml (8 lines)
          extends-unknown/
            moon.yml (5 lines)
          global/
            tasks/
              all.yml (17 lines)
              node-application.yml (10 lines)
              node.yml (9 lines)
              tag-deep1.yml (12 lines)
              tag-deep2.yml (14 lines)
              tag-deep3.yml (12 lines)
              tag-extends.yml (15 lines)
              tag-implicit.yml (9 lines)
              tag-merge.yml (35 lines)
              tag-scope.yml (6 lines)
          global-interweave/
            tasks/
              all.yml (10 lines)
          global-overrides/
            tasks/
              all.yml (10 lines)
          implicits/
            moon.yml (23 lines)
          inheritance/
            moon.yml (8 lines)
          inputs/
            moon.yml (22 lines)
          inputs-project/
            moon.yml (14 lines)
          local/
            moon.yml (24 lines)
          merge-append/
            moon.yml (46 lines)
          merge-prepend/
            moon.yml (46 lines)
          merge-preserve/
            moon.yml (46 lines)
          merge-replace/
            moon.yml (46 lines)
          merge-replace-empty/
            moon.yml (42 lines)
          merge-replace-undefined/
            moon.yml (30 lines)
          no-tasks/
            moon.yml (1 lines)
          options/
            moon.yml (56 lines)
          options-default/
            moon.yml (8 lines)
            package.json (0 lines)
          options-os/
            moon.yml (13 lines)
          override-exclude/
            moon.yml (3 lines)
          override-global/
            moon.yml (11 lines)
          override-include/
            moon.yml (3 lines)
          override-none/
            moon.yml (3 lines)
          override-overlap/
            moon.yml (4 lines)
          override-rename/
            moon.yml (6 lines)
          presets/
            moon.yml (21 lines)
          scopes/
            moon.yml (7 lines)
            package.json (0 lines)
          scripts/
            moon.yml (26 lines)
          syntax/
            moon.yml (52 lines)
          syntax-error/
            moon.yml (4 lines)
          tokens/
            moon.yml (11 lines)
          toolchains/
            moon.yml (31 lines)
          moon.yml (10 lines)
        builder-poly/
          moon.yml (10 lines)
      task_deps_builder_test.rs (212 lines)
      tasks_builder_test.rs (777 lines)
      utils.rs (27 lines)
    Cargo.toml (35 lines)
  task-expander/
    src/
      lib.rs (4 lines)
      task_expander_error.rs (6 lines)
      task_expander.rs (155 lines)
      token_expander_error.rs (5 lines)
      token_expander.rs (404 lines)
    tests/
      __fixtures__/
        file-group/
          project/
            source/
              dir/
                subdir/
                  nested.json (0 lines)
                  not-used.md (0 lines)
              other/
                file.json (0 lines)
              config.yml (0 lines)
              docs.md (0 lines)
      task_expander_test.rs (247 lines)
      token_expander_test.rs (355 lines)
      utils.rs (67 lines)
    Cargo.toml (34 lines)
  task-graph/
    src/
      lib.rs (2 lines)
      task_graph_error.rs (5 lines)
      task_graph.rs (148 lines)
    Cargo.toml (28 lines)
  task-hasher/
    src/
      lib.rs (3 lines)
      task_fingerprint.rs (55 lines)
      task_hasher_error.rs (6 lines)
      task_hasher.rs (155 lines)
    tests/
      __fixtures__/
        ignore-patterns/
          moon.yml (9 lines)
          package.json (1 lines)
        inputs/
          dir/
            abc.txt (0 lines)
            az.txt (0 lines)
            xyz.txt (0 lines)
          1.txt (0 lines)
          2.txt (0 lines)
          3.txt (0 lines)
          moon.yml (71 lines)
          package.json (1 lines)
        output-filters/
          .moon/
            toolchains.yml (1 lines)
            workspace.yml (2 lines)
          moon.yml (48 lines)
          package.json (1 lines)
        projects/
          external/
            data.json (1 lines)
            docs.md (0 lines)
            moon.yml (5 lines)
          inputs/
            moon.yml (17 lines)
      task_hasher_test.rs (188 lines)
    Cargo.toml (35 lines)
  task-runner/
    src/
      command_builder.rs (205 lines)
      command_executor.rs (167 lines)
      lib.rs (9 lines)
      output_archiver.rs (116 lines)
      output_hydrater.rs (77 lines)
      run_state.rs (3 lines)
      task_hashing.rs (217 lines)
      task_runner_error.rs (7 lines)
      task_runner.rs (350 lines)
    tests/
      __fixtures__/
        archive/
          project/
            moon.yml (108 lines)
        builder/
          dotenv/
            .env (3 lines)
            .env.invalid (3 lines)
            .env.local (1 lines)
            .env.unset (1 lines)
            moon.yml (44 lines)
          project/
            input.txt (0 lines)
            moon.yml (10 lines)
          .env.shared (5 lines)
        extension/
          project/
            moon.yml (28 lines)
          script/
            moon.yml (14 lines)
        runner/
          project/
            moon.yml (29 lines)
          unix/
            moon.yml (55 lines)
          windows/
            moon.yml (55 lines)
        toolchain/
          project/
            moon.yml (43 lines)
          script/
            moon.yml (18 lines)
        toolchain-extension/
          project/
            moon.yml (3 lines)
      command_builder_test.rs (326 lines)
      command_executor_test.rs (51 lines)
      output_archiver_test.rs (145 lines)
      output_hydrater_test.rs (49 lines)
      task_runner_test.rs (349 lines)
      utils.rs (111 lines)
    Cargo.toml (50 lines)
  test-utils/
    src/
      lib.rs (4 lines)
      sandbox.rs (93 lines)
      workspace_mocker.rs (247 lines)
    Cargo.toml (36 lines)
  time/
    src/
      lib.rs (54 lines)
    Cargo.toml (16 lines)
  toolchain/
    src/
      lib.rs (47 lines)
      spec.rs (41 lines)
    Cargo.toml (16 lines)
  toolchain-plugin/
    src/
      lib.rs (3 lines)
      toolchain_plugin.rs (284 lines)
      toolchain_registry_actions.rs (159 lines)
      toolchain_registry.rs (137 lines)
    Cargo.toml (30 lines)
  vcs/
    src/
      git/
        common.rs (48 lines)
        git_client.rs (431 lines)
        git_error.rs (6 lines)
        mod.rs (6 lines)
        tree.rs (249 lines)
      changed_files.rs (66 lines)
      lib.rs (7 lines)
      process_cache.rs (74 lines)
      vcs.rs (91 lines)
    tests/
      __fixtures__/
        changed/
          delete-me.txt (0 lines)
          existing.txt (0 lines)
          rename-me.txt (0 lines)
        nested/
          backend/
            file.ts (0 lines)
          frontend/
            file.js (0 lines)
        vcs/
          bar/
            sub/
              dir/
                file4.txt (0 lines)
          baz/
            dir/
              file6.txt (0 lines)
            file5.txt (0 lines)
          foo/
            file1.txt (0 lines)
            file2.txt (0 lines)
            file3.txt (0 lines)
      git_test.rs (268 lines)
    Cargo.toml (39 lines)
  vcs-hooks/
    src/
      hooks_fingerprint.rs (10 lines)
      hooks_generator.rs (143 lines)
      lib.rs (2 lines)
    tests/
      hooks_generator_test.rs (117 lines)
    Cargo.toml (31 lines)
  workspace/
    src/
      build_data.rs (29 lines)
      lib.rs (7 lines)
      projects_locator.rs (81 lines)
      repo_type.rs (5 lines)
      tasks_querent.rs (29 lines)
      workspace_builder_error.rs (5 lines)
      workspace_builder.rs (590 lines)
      workspace_cache.rs (47 lines)
    Cargo.toml (41 lines)
  workspace-graph/
    src/
      lib.rs (87 lines)
      query_projects.rs (97 lines)
      query_tasks.rs (66 lines)
    Cargo.toml (22 lines)
docs/
  CHANGELOG_V0.md (1109 lines)
  CHANGELOG_V1.md (3436 lines)
  PLUGIN_APIS.md (39 lines)
  RELEASE_CRATES.md (10 lines)
legacy/
  core/
    test-utils/
      src/
        cli.rs (114 lines)
        configs.rs (309 lines)
        lib.rs (34 lines)
        sandbox.rs (111 lines)
      Cargo.toml (20 lines)
packages/
  cli/
    LICENSE (18 lines)
    moon.js (0 lines)
    moonx.js (0 lines)
    package.json (40 lines)
    README.md (104 lines)
    utils.js (10 lines)
  core-linux-arm64-gnu/
    package.json (36 lines)
    README.md (3 lines)
  core-linux-arm64-musl/
    package.json (36 lines)
    README.md (3 lines)
  core-linux-x64-gnu/
    package.json (36 lines)
    README.md (3 lines)
  core-linux-x64-musl/
    package.json (36 lines)
    README.md (3 lines)
  core-macos-arm64/
    package.json (33 lines)
    README.md (3 lines)
  core-windows-x64-msvc/
    package.json (33 lines)
    README.md (3 lines)
  report/
    src/
      action.ts (17 lines)
      index.ts (0 lines)
      report.ts (16 lines)
      time.ts (19 lines)
    tests/
      action.test.ts (2 lines)
      report.test.ts (4 lines)
      time.test.ts (1 lines)
    LICENSE (18 lines)
    moon.yml (2 lines)
    package.json (44 lines)
    README.md (7 lines)
    tsconfig.cjs.json (10 lines)
    tsconfig.json (19 lines)
  runtime/
    src/
      context.ts (12 lines)
      index.ts (0 lines)
    LICENSE (18 lines)
    moon.yml (4 lines)
    package.json (46 lines)
    README.md (9 lines)
    tsconfig.cjs.json (10 lines)
    tsconfig.json (17 lines)
  types/
    src/
      common.ts (26 lines)
      events.ts (183 lines)
      extensions-config.ts (44 lines)
      index.ts (0 lines)
      mcp.ts (62 lines)
      pipeline.ts (237 lines)
      project-config.ts (807 lines)
      project.ts (49 lines)
      task.ts (106 lines)
      tasks-config.ts (1766 lines)
      template-config.ts (533 lines)
      toolchains-config.ts (234 lines)
      workspace-config.ts (2068 lines)
    LICENSE (18 lines)
    moon.yml (2 lines)
    package.json (41 lines)
    README.md (7 lines)
    tsconfig.cjs.json (10 lines)
    tsconfig.json (15 lines)
  visualizer/
    src/
      components/
        Graph.tsx (9 lines)
      helpers/
        render.ts (32 lines)
        types.ts (16 lines)
      app.css (0 lines)
      app.tsx (12 lines)
      index.css (3 lines)
      main.tsx (7 lines)
      vite-env.d.ts (1 lines)
    index.html (18 lines)
    LICENSE (18 lines)
    moon.yml (36 lines)
    package.json (43 lines)
    postcss.config.cjs (0 lines)
    README.md (7 lines)
    tailwind.config.cjs (1 lines)
    tsconfig.json (25 lines)
    vite.config.mjs (4 lines)
scenarios/
  bash-lang/
    moon.yml (3 lines)
  deno-signals/
    main.ts (3 lines)
    moon.yml (4 lines)
  interactive/
    input.go (19 lines)
    input.mjs (0 lines)
    input.py (1 lines)
    moon.yml (30 lines)
  js-platforms/
    moon.yml (6 lines)
    tsconfig.json (1 lines)
  python/
    src/
      main.py (0 lines)
    moon.yml (3 lines)
    pyproject.toml (4 lines)
    requirements.in (0 lines)
  signals/
    ctrlc.mjs (0 lines)
    exitCode.mjs (0 lines)
    moon.yml (23 lines)
    signals.mjs (3 lines)
scripts/
  data/
    generateCerts.sh (48 lines)
  release/
    buildPackages.sh (15 lines)
    release.sh (131 lines)
    setupNpm.sh (14 lines)
    tag.sh (24 lines)
  planTestCi.sh (18 lines)
tests/
  docker/
    Dockerfile (31 lines)
    Dockerfile.staged (44 lines)
  fixtures/
    archives/
      folder/
        nested/
          other.js (0 lines)
        file.js (0 lines)
      file.txt (0 lines)
    base/
      files-and-dirs/
        dir/
          subdir/
            another.ts (0 lines)
          other.tsx (0 lines)
        file.ts (0 lines)
        README.md (0 lines)
      other/
        outside.ts (0 lines)
      tsconfig-json/
        a/
          tsconfig.json (9 lines)
        b/
          tsconfig.json (8 lines)
        tsconfig.common.json (53 lines)
        tsconfig.complete.json (106 lines)
        tsconfig.default.json (71 lines)
        tsconfig.inherits.json (8 lines)
        tsconfig.multi-inherits.json (8 lines)
      package.json (4 lines)
      README.md (0 lines)
    bun/
      base/
        affectedFiles.js (0 lines)
        args.js (0 lines)
        binExecArgs.js (0 lines)
        cjsFile.cjs (0 lines)
        cwd.js (0 lines)
        envVars.js (0 lines)
        envVarsMoon.js (0 lines)
        execBin.js (0 lines)
        exitCodeNonZero.js (0 lines)
        exitCodeZero.js (0 lines)
        mjsFile.mjs (0 lines)
        moon.yml (87 lines)
        package.json (7 lines)
        processExitNonZero.js (0 lines)
        processExitZero.js (0 lines)
        standard.js (0 lines)
        throwError.js (0 lines)
        topLevelAwait.mjs (0 lines)
        unhandledPromise.js (0 lines)
      package-manager/
        moon.yml (11 lines)
        package.json (10 lines)
      scripts/
        moon.yml (1 lines)
        package.json (8 lines)
      version-override/
        moon.yml (5 lines)
        package.json (4 lines)
      .bunrc (0 lines)
      package.json (10 lines)
    bun-node-pm/
      .moon/
        toolchains.yml (8 lines)
        workspace.yml (3 lines)
      bun/
        .bunrc (0 lines)
        moon.yml (5 lines)
        package.json (4 lines)
      node/
        moon.yml (5 lines)
        package.json (4 lines)
      package.json (8 lines)
    cases/
      affected/
        affected.js (0 lines)
        moon.yml (20 lines)
      base/
        moon.yml (18 lines)
      depends-on/
        moon.yml (46 lines)
      deps-a/
        moon.yml (18 lines)
      deps-b/
        moon.yml (18 lines)
      deps-c/
        moon.yml (16 lines)
      files/
        affected.js (0 lines)
        file.txt (0 lines)
        moon.yml (8 lines)
      interactive/
        moon.yml (10 lines)
        prompt.js (0 lines)
      mutex/
        moon.yml (16 lines)
        sleep.mjs (0 lines)
      no-affected/
        affected.js (0 lines)
        file.txt (0 lines)
        moon.yml (18 lines)
      noop/
        moon.yml (9 lines)
      output-styles/
        moon.yml (62 lines)
        style.js (0 lines)
      outputs/
        .env (1 lines)
        generate.js (1 lines)
        moon.yml (127 lines)
      outputs-filtering/
        moon.yml (47 lines)
      passthrough-args/
        moon.yml (15 lines)
        passthroughArgs.sh (4 lines)
      states/
        moon.yml (9 lines)
      target-scope-a/
        moon.yml (17 lines)
      target-scope-b/
        moon.yml (14 lines)
      target-scope-c/
        moon.yml (6 lines)
      task-deps/
        moon.yml (19 lines)
        output.js (0 lines)
      task-os/
        moon.yml (13 lines)
      task-script/
        args.sh (2 lines)
        moon.yml (13 lines)
      moon.yml (14 lines)
      package.json (7 lines)
      tsconfig.json (7 lines)
    deno/
      base/
        affectedFiles.ts (0 lines)
        args.ts (0 lines)
        cwd.ts (0 lines)
        envVars.ts (0 lines)
        envVarsMoon.ts (0 lines)
        exitCodeNonZero.ts (0 lines)
        exitCodeZero.ts (0 lines)
        moon.yml (53 lines)
        standard.ts (0 lines)
        throwError.ts (0 lines)
        topLevelAwait.ts (0 lines)
        unhandledPromise.ts (0 lines)
      version-override/
        moon.yml (5 lines)
      deno.json (1 lines)
    editor-config/
      .editorconfig (4 lines)
      file.json (8 lines)
      file.yaml (7 lines)
    empty/
      empty (0 lines)
    generator/
      templates/
        configs/
          file.json (7 lines)
          file.yaml (4 lines)
          template.yml (4 lines)
        dest/
          file.txt (0 lines)
          template.yml (9 lines)
        extends/
          base.txt (0 lines)
          template.yml (9 lines)
          two.txt (1 lines)
          vars.txt (2 lines)
        extends-one/
          one.txt (1 lines)
          template.yml (9 lines)
        extends-two/
          template.yml (8 lines)
          two.txt (1 lines)
        extensions/
          file.ts.tera (1 lines)
          file.tsx.twig (1 lines)
          template.yml (9 lines)
        frontmatter/
          component.tsx (0 lines)
          forced.txt (5 lines)
          skipped.txt (5 lines)
          template.yml (4 lines)
          to.txt (5 lines)
        standard/
          folder/
            nested-file.ts (0 lines)
          file.ts (0 lines)
          file.txt (2 lines)
          other.raw.txt (2 lines)
          template.yml (4 lines)
        vars/
          control.txt (18 lines)
          expressions.txt (34 lines)
          file-[stringNotEmpty]-[number].txt (0 lines)
          filters.txt (29 lines)
          partial.txt (1 lines)
          template.yml (134 lines)
        vars-collections/
          print.txt (2 lines)
          template.yml (15 lines)
      package.json (4 lines)
    ignore/
      dir/
        qux (1 lines)
      .gitignore (1 lines)
      foo (1 lines)
    init-sandbox/
      package.json (4 lines)
    migrate/
      package-json/
        common/
          package.json (8 lines)
        deps/
          package.json (11 lines)
      .gitignore (3 lines)
    node/
      base/
        affectedFiles.js (0 lines)
        args.js (0 lines)
        binExecArgs.js (0 lines)
        cjsFile.cjs (0 lines)
        ctrlc.js (0 lines)
        cwd.js (0 lines)
        envVars.js (0 lines)
        envVarsMoon.js (0 lines)
        execBin.js (0 lines)
        exitCodeNonZero.js (0 lines)
        exitCodeZero.js (0 lines)
        mjsFile.mjs (0 lines)
        moon.yml (95 lines)
        package.json (7 lines)
        processExitNonZero.js (0 lines)
        processExitZero.js (0 lines)
        standard.js (0 lines)
        throwError.js (0 lines)
        topLevelAwait.mjs (0 lines)
        unhandledPromise.js (0 lines)
      depends-on/
        moon.yml (21 lines)
        package.json (6 lines)
        tsconfig.json (3 lines)
      depends-on-scopes/
        moon.yml (15 lines)
        package.json (3 lines)
      deps-a/
        moon.yml (16 lines)
        package.json (4 lines)
      deps-b/
        moon.yml (16 lines)
        package.json (4 lines)
        tsconfig.json (3 lines)
      deps-c/
        moon.yml (14 lines)
        tsconfig.json (3 lines)
      deps-d/
        package.json (4 lines)
      esbuild/
        input.js (0 lines)
        moon.yml (10 lines)
        package.json (7 lines)
      lifecycles/
        package.json (7 lines)
        postinstall.mjs (0 lines)
      postinstall/
        package.json (7 lines)
        postinstall.js (0 lines)
      postinstall-recursion/
        package.json (7 lines)
        postinstall.js (1 lines)
      swc/
        input.js (0 lines)
        moon.yml (10 lines)
        package.json (13 lines)
      version-override/
        moon.yml (5 lines)
        package.json (4 lines)
      package.json (10 lines)
    node-bun/
      project/
        moon.yml (5 lines)
        package.json (7 lines)
      workspaces/
        base/
          moon.yml (15 lines)
          package.json (10 lines)
        not-in-workspace/
          package.json (7 lines)
        other/
          package.json (10 lines)
        package.json (11 lines)
    node-non-workspaces/
      bar/
        package.json (4 lines)
      baz/
        package.json (4 lines)
      foo/
        package.json (4 lines)
      package.json (4 lines)
    node-npm/
      project/
        moon.yml (5 lines)
        package.json (7 lines)
      workspaces/
        base/
          moon.yml (15 lines)
          package.json (10 lines)
        not-in-workspace/
          package.json (7 lines)
        other/
          package.json (10 lines)
        package.json (11 lines)
    node-pnpm/
      project/
        .npmrc (2 lines)
        moon.yml (5 lines)
        package.json (7 lines)
      workspaces/
        base/
          moon.yml (15 lines)
          package.json (10 lines)
        not-in-workspace/
          package.json (7 lines)
        other/
          package.json (10 lines)
        .npmrc (2 lines)
        package.json (7 lines)
        pnpm-workspace.yaml (3 lines)
    node-yarn/
      project/
        .yarnrc.yml (1 lines)
        moon.yml (5 lines)
        package.json (7 lines)
      workspaces/
        base/
          moon.yml (15 lines)
          package.json (10 lines)
        not-in-workspace/
          package.json (7 lines)
        other/
          package.json (10 lines)
        .yarnrc.yml (1 lines)
        package.json (11 lines)
    node-yarn1/
      project/
        moon.yml (5 lines)
        package.json (7 lines)
      workspaces/
        base/
          moon.yml (16 lines)
          package.json (10 lines)
        not-in-workspace/
          package.json (7 lines)
        other/
          package.json (10 lines)
        package.json (11 lines)
    project-graph/
      aliases/
        explicit/
          moon.yml (5 lines)
          package.json (3 lines)
        explicit-and-implicit/
          moon.yml (5 lines)
          package.json (6 lines)
        implicit/
          package.json (9 lines)
        no-lang/
          moon.yml (3 lines)
        node/
          moon.yml (13 lines)
          package.json (3 lines)
        node-name-only/
          moon.yml (6 lines)
          package.json (3 lines)
        node-name-scope/
          moon.yml (6 lines)
          package.json (3 lines)
        package.json (5 lines)
      dependencies/
        a/
          moon.yml (2 lines)
        b/
          moon.yml (2 lines)
        c/
          moon.yml (1 lines)
        d/
          moon.yml (1 lines)
        package.json (5 lines)
    projects/
      advanced/
        moon.yml (5 lines)
      basic/
        file.ts (0 lines)
        moon.yml (10 lines)
      deps/
        bar/
          empty (0 lines)
        baz/
          empty (0 lines)
        foo/
          moon.yml (8 lines)
      empty-config/
        moon.yml (1 lines)
      metadata/
        moon.yml (18 lines)
      no-config/
        empty (0 lines)
      package-json/
        package.json (7 lines)
      tasks/
        moon.yml (21 lines)
      toolchains/
        moon.yml (9 lines)
      package.json (5 lines)
    python/
      base/
        .gitignore (1 lines)
        moon.yml (12 lines)
    python-uv/
      base/
        .gitignore (1 lines)
        moon.yml (12 lines)
        pyproject.toml (6 lines)
    rust/
      cases/
        src/
          bin/
            args.rs (5 lines)
            cwd.rs (4 lines)
            env_vars_moon.rs (13 lines)
            env_vars.rs (9 lines)
            exit_nonzero.rs (5 lines)
            exit_zero.rs (5 lines)
            panic.rs (2 lines)
            standard.rs (3 lines)
          lib.rs (0 lines)
        Cargo.toml (4 lines)
        moon.yml (53 lines)
      project/
        src/
          lib.rs (0 lines)
        Cargo.toml (7 lines)
        moon.yml (1 lines)
      toolchain/
        src/
          lib.rs (0 lines)
        Cargo.toml (4 lines)
        moon.yml (7 lines)
      workspaces/
        crates/
          bin-crate/
            src/
              main.rs (1 lines)
            Cargo.toml (4 lines)
          excluded-member/
            src/
              lib.rs (0 lines)
            Cargo.toml (4 lines)
          inherited-dep/
            src/
              lib.rs (0 lines)
            Cargo.toml (7 lines)
          normal-dep/
            src/
              lib.rs (0 lines)
            Cargo.toml (7 lines)
          path-deps/
            src/
              lib.rs (0 lines)
            Cargo.toml (8 lines)
        Cargo.toml (7 lines)
        moon.yml (1 lines)
    system/
      unix/
        affectedFiles.sh (5 lines)
        args.sh (6 lines)
        cwd.sh (4 lines)
        envVars.sh (6 lines)
        envVarsMoon.sh (8 lines)
        exitNonZero.sh (9 lines)
        exitZero.sh (9 lines)
        moon.yml (168 lines)
        outputs.sh (6 lines)
        standard.sh (5 lines)
      windows/
        cwd.bat (1 lines)
        echo.bat (1 lines)
        echo.ps1 (1 lines)
        envVars.bat (3 lines)
        envVarsMoon.bat (4 lines)
        exitNonZero.bat (6 lines)
        exitZero.bat (6 lines)
        moon.yml (118 lines)
        outputs.bat (3 lines)
        passthroughArgs.bat (1 lines)
        standard.bat (2 lines)
    task-inheritance/
      exclude/
        moon.yml (3 lines)
      exclude-all/
        moon.yml (3 lines)
      exclude-none/
        moon.yml (3 lines)
      include/
        moon.yml (3 lines)
      include-exclude/
        moon.yml (4 lines)
      include-exclude-rename/
        moon.yml (6 lines)
      include-none/
        moon.yml (3 lines)
      inputs/
        moon.yml (12 lines)
      platform-detect/
        moon.yml (8 lines)
      platform-detect-lang/
        moon.yml (10 lines)
      rename/
        moon.yml (6 lines)
      rename-merge/
        moon.yml (9 lines)
    tasks/
      basic/
        moon.yml (9 lines)
      build-a/
        moon.yml (14 lines)
      build-b/
        moon.yml (5 lines)
      build-c/
        moon.yml (5 lines)
      chain/
        moon.yml (20 lines)
      cycle/
        moon.yml (12 lines)
      expand-env/
        .env (2 lines)
        .env.production (2 lines)
        .env.subs (6 lines)
        moon.yml (26 lines)
      expand-env-project/
        .env (2 lines)
        moon.yml (20 lines)
      expand-outputs/
        moon.yml (2 lines)
      file-groups/
        moon.yml (4 lines)
      inherit-tags/
        moon.yml (5 lines)
      input-a/
        a.ts (0 lines)
        a2.ts (0 lines)
        moon.yml (11 lines)
      input-b/
        b.ts (0 lines)
        b2.ts (0 lines)
        moon.yml (11 lines)
      input-c/
        c.ts (0 lines)
        moon.yml (6 lines)
      inputs/
        moon.yml (11 lines)
      interactive/
        moon.yml (17 lines)
      merge-all-strategies/
        a.ts (0 lines)
        b.ts (0 lines)
        moon.yml (18 lines)
      merge-append/
        a.ts (0 lines)
        b.ts (0 lines)
        moon.yml (19 lines)
      merge-prepend/
        a.ts (0 lines)
        b.ts (0 lines)
        moon.yml (20 lines)
      merge-replace/
        a.ts (0 lines)
        b.ts (0 lines)
        moon.yml (20 lines)
      no-tasks/
        moon.yml (3 lines)
      persistent/
        moon.yml (22 lines)
      scope-all/
        moon.yml (2 lines)
      scope-deps/
        moon.yml (18 lines)
      scope-self/
        moon.yml (21 lines)
      tokens/
        dir/
          subdir/
            another.ts (0 lines)
          other.tsx (0 lines)
        file.ts (0 lines)
        moon.yml (78 lines)
      .env (1 lines)
      package.json (5 lines)
    typescript/
      base-no-src/
        index.tsx (0 lines)
        moon.yml (1 lines)
        package.json (4 lines)
      base-src/
        src/
          index.ts (0 lines)
        moon.yml (1 lines)
        package.json (4 lines)
      create-config/
        moon.yml (1 lines)
        package.json (4 lines)
      create-config-disabled/
        moon.yml (2 lines)
        package.json (4 lines)
      deps-no-config/
        moon.yml (1 lines)
        package.json (4 lines)
      deps-no-config-disabled/
        moon.yml (2 lines)
        package.json (4 lines)
      deps-with-config/
        package.json (4 lines)
        tsconfig.json (3 lines)
      deps-with-config-disabled/
        moon.yml (2 lines)
        package.json (4 lines)
        tsconfig.json (3 lines)
      out-dir-routing/
        moon.yml (1 lines)
        package.json (4 lines)
        tsconfig.json (5 lines)
      out-dir-routing-no-options/
        moon.yml (1 lines)
        package.json (4 lines)
        tsconfig.json (1 lines)
      out-dir-routing-project-disabled/
        moon.yml (3 lines)
        package.json (4 lines)
        tsconfig.json (5 lines)
      syncs-deps-refs/
        moon.yml (6 lines)
        package.json (4 lines)
      syncs-deps-refs-project-disabled/
        moon.yml (9 lines)
        package.json (4 lines)
        tsconfig.json (1 lines)
      syncs-paths-refs/
        moon.yml (3 lines)
        package.json (4 lines)
        tsconfig.json (1 lines)
      syncs-paths-refs-project-disabled/
        moon.yml (7 lines)
        package.json (4 lines)
        tsconfig.json (1 lines)
      moon.yml (2 lines)
      package.json (7 lines)
      tsconfig.json (3 lines)
    vcs/
      delete-me.txt (0 lines)
      existing.txt (0 lines)
      rename-me.txt (0 lines)
  repros/
    1174/
      .moon/
        tasks/
          tag-component.yml (35 lines)
        toolchains.yml (63 lines)
        workspace.yml (2 lines)
      base/
        src/
          index.ts (0 lines)
        moon.yml (8 lines)
        package.json (4 lines)
        tsconfig.json (6 lines)
      chat/
        src/
          index.ts (0 lines)
        moon.yml (8 lines)
        package.json (15 lines)
        tsconfig.json (6 lines)
      container/
        src/
          index.ts (0 lines)
        moon.yml (8 lines)
        package.json (4 lines)
        tsconfig.json (6 lines)
      .gitignore (1 lines)
      package.json (10 lines)
      pnpm-workspace.yaml (4 lines)
    root-peer-deps-sync/
      .moon/
        toolchains.yml (1 lines)
        workspace.yml (3 lines)
      project/
        moon.yml (5 lines)
        package.json (3 lines)
      .gitignore (1 lines)
      moon.yml (7 lines)
      package.json (6 lines)
wasm/
  ext-sync/
    src/
      lib.rs (19 lines)
    Cargo.toml (16 lines)
  ext-task/
    src/
      lib.rs (38 lines)
    Cargo.toml (16 lines)
  tc-tier1/
    src/
      lib.rs (38 lines)
    Cargo.toml (16 lines)
  tc-tier2/
    src/
      lib.rs (68 lines)
    Cargo.toml (17 lines)
  tc-tier2-reqs/
    src/
      lib.rs (5 lines)
    Cargo.toml (17 lines)
  tc-tier2-setup-env/
    src/
      lib.rs (3 lines)
    Cargo.toml (17 lines)
  tc-tier3/
    src/
      lib.rs (25 lines)
    Cargo.toml (17 lines)
  tc-tier3-reqs/
    src/
      lib.rs (5 lines)
    Cargo.toml (17 lines)
  test-plugin/
    src/
      lib.rs (6 lines)
    Cargo.toml (13 lines)
  Cargo.toml (20 lines)
website/
  blog/
    2022-09-01_v0.13.mdx (96 lines)
    2022-09-13_v0.14.mdx (72 lines)
    2022-09-26_v0.15.mdx (176 lines)
    2022-10-06_v0.16.mdx (137 lines)
    2022-10-17_vscode-extension.mdx (30 lines)
    2022-10-21_v0.17.mdx (205 lines)
    2022-10-31_v0.18.mdx (89 lines)
    2022-11-14_v0.19.mdx (102 lines)
    2022-11-21_typescript-monorepo.mdx (19 lines)
    2022-11-29_v0.20.mdx (143 lines)
    2022-12-19_v0.21.mdx (146 lines)
    2023-01-04_2023-roadmap.mdx (152 lines)
    2023-01-16_v0.22.mdx (98 lines)
    2023-01-30_v0.23.mdx (320 lines)
    2023-02-08_moonbase.mdx (57 lines)
    2023-02-13_v0.24.mdx (174 lines)
    2023-02-27_v0.25.mdx (183 lines)
    2023-03-09_proto.mdx (89 lines)
    2023-03-13_v0.26.mdx (152 lines)
    2023-03-15_proto-v0.3.mdx (57 lines)
    2023-03-27_moon-v1.0.mdx (186 lines)
    2023-03-31_proto-v0.4.mdx (58 lines)
    2023-04-03_moon-v1.1.mdx (98 lines)
    2023-04-06_proto-v0.5.mdx (83 lines)
    2023-04-13_proto-v0.6.mdx (88 lines)
    2023-04-17_moon-v1.2.mdx (82 lines)
    2023-04-21_proto-v0.7.mdx (96 lines)
    2023-04-24_moon-v1.3.mdx (85 lines)
    2023-04-28_proto-v0.8.mdx (46 lines)
    2023-05-01_moon-v1.4.mdx (70 lines)
    2023-05-08_moon-v1.5.mdx (120 lines)
    2023-05-15_moon-v1.6.mdx (114 lines)
    2023-05-23_proto-v0.9.mdx (81 lines)
    2023-05-30_moon-v1.7.mdx (88 lines)
    2023-06-12_moon-v1.8.mdx (141 lines)
    2023-06-25_proto-v0.11.mdx (39 lines)
    2023-06-26_moon-v1.9.mdx (96 lines)
    2023-07-07_proto-v0.12.mdx (97 lines)
    2023-07-10_moon-v1.10.mdx (130 lines)
    2023-07-21_proto-v0.13.mdx (88 lines)
    2023-07-31_moon-v1.11.mdx (122 lines)
    2023-08-11_proto-v0.14.mdx (73 lines)
    2023-08-21_moon-v1.12.mdx (158 lines)
    2023-08-23_proto-v0.15.mdx (74 lines)
    2023-09-04_proto-v0.16.mdx (63 lines)
    2023-09-05_moon-v1.13.mdx (84 lines)
    2023-09-11_proto-v0.17.mdx (84 lines)
    2023-09-18_proto-v0.18.mdx (78 lines)
    2023-09-25_moon-v1.14.mdx (132 lines)
    2023-09-29_proto-v0.19.mdx (78 lines)
    2023-10-09_moon-v1.15.mdx (159 lines)
    2023-10-20_proto-v0.20.mdx (92 lines)
    2023-10-27_proto-v0.21.mdx (30 lines)
    2023-10-30_moon-v1.16.mdx (113 lines)
    2023-11-16_proto-v0.23.mdx (80 lines)
    2023-11-20_moon-v1.17.mdx (179 lines)
    2023-12-07_proto-v0.24.mdx (146 lines)
    2023-12-11_proto-v0.25.mdx (29 lines)
    2023-12-12_moon-v1.18.mdx (147 lines)
    2023-12-19_proto-v0.26-rc.mdx (85 lines)
    2023-12-21_proto-v0.26.mdx (71 lines)
    2024-01-01_moon-v1.19.mdx (143 lines)
    2024-01-04_proto-v0.27.mdx (77 lines)
    2024-01-12_2024-roadmap.mdx (191 lines)
    2024-01-17_proto-v0.28.mdx (33 lines)
    2024-01-23_proto-v0.29.mdx (54 lines)
    2024-01-26_moon-v1.20.mdx (121 lines)
    2024-02-07_moon-v1.21.mdx (129 lines)
    2024-02-26_moon-v1.22.mdx (113 lines)
    2024-03-01_proto-v0.31.mdx (105 lines)
    2024-03-25_moon-v1.23.mdx (105 lines)
    2024-04-07_proto-v0.34.mdx (90 lines)
    2024-04-17_moon-v1.24.mdx (101 lines)
    2024-05-05_proto-v0.35.mdx (48 lines)
    2024-05-27_moon-v1.25.mdx (194 lines)
    2024-06-03_proto-v0.36.mdx (72 lines)
    2024-06-16_proto-v0.37.mdx (67 lines)
    2024-06-24_moon-v1.26.mdx (75 lines)
    2024-07-07_proto-v0.38.mdx (62 lines)
    2024-07-14_moon-v1.27.mdx (174 lines)
    2024-07-26_proto-v0.39.mdx (81 lines)
    2024-08-16_proto-v0.40.mdx (76 lines)
    2024-09-02_moon-v1.28.mdx (164 lines)
    2024-10-07_moon-v1.29.mdx (356 lines)
    2024-10-31_proto-v0.42.mdx (107 lines)
    2024-11-25_moon-v1.30.mdx (163 lines)
    2024-12-25_proto-v0.44.mdx (96 lines)
    2025-01-06_moon-v1.31.mdx (202 lines)
    2025-01-22_proto-v0.45.mdx (113 lines)
    2025-02-03_moon-v1.32.mdx (188 lines)
    2025-02-25_proto-v0.47.mdx (86 lines)
    2025-03-13_moon-v1.33.mdx (158 lines)
    2025-03-31_moon-v1.34.mdx (159 lines)
    2025-03-31_moonbase-sunset.mdx (26 lines)
    2025-04-16_moon-v1.35.mdx (116 lines)
    2025-05-18_moon-v1.36.mdx (135 lines)
    2025-06-03_moon-v1.37.mdx (124 lines)
    2025-06-11_proto-v0.50.mdx (85 lines)
    2025-06-24_moon-v1.38.mdx (105 lines)
    2025-07-17_proto-v0.51.mdx (169 lines)
    2025-07-24_moon-v1.39.mdx (162 lines)
    2025-08-21_proto-v0.52.mdx (62 lines)
    2025-09-01_moon-v1.40.mdx (255 lines)
    2025-09-18_proto-v0.53.mdx (83 lines)
    2025-09-28_moon-v1.41.mdx (161 lines)
    2026-01-01_moon-v2-alpha.mdx (70 lines)
    2026-01-13_moon-v2-beta.mdx (57 lines)
    2026-01-26_moon-v2-rc.mdx (69 lines)
    2026-02-01_moon-v2.0.mdx (474 lines)
    authors.yml (10 lines)
  docs/
    __partials__/
      create-task/
        bun/
          args.mdx (10 lines)
          base.mdx (10 lines)
          filegroups.mdx (19 lines)
          inputs.mdx (14 lines)
          outputs.mdx (16 lines)
        deno/
          args.mdx (7 lines)
          base.mdx (7 lines)
          filegroups.mdx (15 lines)
          inputs.mdx (10 lines)
          outputs.mdx (12 lines)
        go/
          args.mdx (7 lines)
          base.mdx (7 lines)
          filegroups.mdx (14 lines)
          inputs.mdx (9 lines)
          outputs.mdx (11 lines)
        node/
          args.mdx (7 lines)
          base.mdx (7 lines)
          filegroups.mdx (16 lines)
          inputs.mdx (11 lines)
          outputs.mdx (13 lines)
        php/
          args.mdx (7 lines)
          base.mdx (14 lines)
          filegroups.mdx (16 lines)
          inputs.mdx (11 lines)
          outputs.mdx (13 lines)
        python/
          args.mdx (7 lines)
          base.mdx (14 lines)
          filegroups.mdx (16 lines)
          inputs.mdx (11 lines)
          outputs.mdx (13 lines)
        ruby/
          args.mdx (7 lines)
          base.mdx (14 lines)
          filegroups.mdx (16 lines)
          inputs.mdx (11 lines)
          outputs.mdx (13 lines)
        rust/
          args.mdx (7 lines)
          base.mdx (7 lines)
          filegroups.mdx (16 lines)
          inputs.mdx (11 lines)
          outputs.mdx (13 lines)
      migrate/
        bun/
          migrate.mdx (43 lines)
          scripts.mdx (21 lines)
        deno/
          migrate.mdx (39 lines)
          scripts.mdx (22 lines)
        go/
          migrate.mdx (39 lines)
          scripts.mdx (2 lines)
        node/
          migrate.mdx (51 lines)
          scripts.mdx (24 lines)
        php/
          migrate.mdx (34 lines)
          scripts.mdx (22 lines)
        python/
          migrate.mdx (43 lines)
          scripts.mdx (19 lines)
        ruby/
          migrate.mdx (39 lines)
          scripts.mdx (19 lines)
        rust/
          migrate.mdx (47 lines)
          scripts.mdx (3 lines)
      node/
        package-workspaces.mdx (66 lines)
      setup-toolchain/
        bun/
          tier2.mdx (6 lines)
          tier3.mdx (7 lines)
        deno/
          tier2.mdx (6 lines)
          tier3.mdx (7 lines)
        go/
          tier2.mdx (3 lines)
          tier3.mdx (4 lines)
        node/
          tier2.mdx (8 lines)
          tier3.mdx (10 lines)
        php/
          tier2.mdx (6 lines)
          tier3.mdx (6 lines)
        python/
          tier2.mdx (6 lines)
          tier3.mdx (7 lines)
        ruby/
          tier2.mdx (6 lines)
          tier3.mdx (6 lines)
        rust/
          tier2.mdx (3 lines)
          tier3.mdx (4 lines)
    commands/
      docker/
        file.mdx (59 lines)
        prune.mdx (30 lines)
        scaffold.mdx (112 lines)
        setup.mdx (30 lines)
      extension/
        add.mdx (38 lines)
        info.mdx (56 lines)
      query/
        affected.mdx (33 lines)
        changed-files.mdx (52 lines)
        projects.mdx (80 lines)
        tasks.mdx (62 lines)
      sync/
        code-owners.mdx (26 lines)
        config-schemas.mdx (19 lines)
        projects.mdx (30 lines)
        vcs-hooks.mdx (26 lines)
      toolchain/
        add.mdx (38 lines)
        info.mdx (124 lines)
      action-graph.mdx (58 lines)
      bin.mdx (20 lines)
      check.mdx (42 lines)
      ci.mdx (40 lines)
      clean.mdx (19 lines)
      completions.mdx (73 lines)
      exec.mdx (77 lines)
      ext.mdx (34 lines)
      generate.mdx (42 lines)
      hash.mdx (86 lines)
      init.mdx (24 lines)
      mcp.mdx (21 lines)
      overview.mdx (156 lines)
      project-graph.mdx (51 lines)
      project.mdx (76 lines)
      projects.mdx (29 lines)
      run.mdx (54 lines)
      setup.mdx (21 lines)
      task-graph.mdx (49 lines)
      task.mdx (74 lines)
      tasks.mdx (38 lines)
      teardown.mdx (17 lines)
      template.mdx (38 lines)
      templates.mdx (24 lines)
      upgrade.mdx (17 lines)
    concepts/
      cache.mdx (93 lines)
      file-group.mdx (59 lines)
      file-pattern.mdx (67 lines)
      project.mdx (53 lines)
      query-lang.mdx (169 lines)
      target.mdx (140 lines)
      task-inheritance.mdx (155 lines)
      task.mdx (143 lines)
      token.mdx (518 lines)
      toolchain.mdx (73 lines)
      workspace.mdx (14 lines)
    config/
      extensions.mdx (64 lines)
      overview.mdx (95 lines)
      project.mdx (1856 lines)
      tasks.mdx (199 lines)
      template.mdx (342 lines)
      toolchain.mdx (178 lines)
      workspace.mdx (1004 lines)
    editors/
      vscode.mdx (94 lines)
    guides/
      examples/
        angular.mdx (223 lines)
        astro.mdx (129 lines)
        eslint.mdx (225 lines)
        jest.mdx (111 lines)
        nest.mdx (81 lines)
        next.mdx (166 lines)
        nuxt.mdx (114 lines)
        packemon.mdx (87 lines)
        prettier.mdx (96 lines)
        react.mdx (20 lines)
        remix.mdx (116 lines)
        solid.mdx (60 lines)
        storybook.mdx (155 lines)
        sveltekit.mdx (157 lines)
        typescript.mdx (161 lines)
        vite.mdx (72 lines)
        vue.mdx (92 lines)
      javascript/
        __partials__/
          workspace-commands.mdx (154 lines)
        bun-handbook.mdx (131 lines)
        deno-handbook.mdx (66 lines)
        node-handbook.mdx (403 lines)
        typescript-eslint.mdx (73 lines)
        typescript-project-refs.mdx (766 lines)
      rust/
        handbook.mdx (336 lines)
      ci.mdx (300 lines)
      codegen.mdx (401 lines)
      codeowners.mdx (218 lines)
      debug-task.mdx (177 lines)
      docker.mdx (384 lines)
      extensions.mdx (301 lines)
      mcp.mdx (146 lines)
      notifications.mdx (55 lines)
      offline-mode.mdx (37 lines)
      open-source.mdx (76 lines)
      pkl-config.mdx (191 lines)
      profile.mdx (134 lines)
      remote-cache.mdx (152 lines)
      root-project.mdx (73 lines)
      sharing-config.mdx (48 lines)
      vcs-hooks.mdx (167 lines)
      wasm-plugins.mdx (524 lines)
      webhooks.mdx (570 lines)
    how-it-works/
      action-graph.mdx (123 lines)
      languages.mdx (145 lines)
      project-graph.mdx (89 lines)
      task-graph.mdx (43 lines)
    migrate/
      2.0.mdx (570 lines)
    proto/
      commands/
        debug/
          config.mdx (51 lines)
          env.mdx (45 lines)
        plugin/
          add.mdx (29 lines)
          info.mdx (74 lines)
          list.mdx (64 lines)
          remove.mdx (28 lines)
          search.mdx (34 lines)
        activate.mdx (113 lines)
        alias.mdx (28 lines)
        bin.mdx (40 lines)
        clean.mdx (29 lines)
        completions.mdx (74 lines)
        diagnose.mdx (29 lines)
        exec.mdx (58 lines)
        install.mdx (83 lines)
        list-remote.mdx (43 lines)
        list.mdx (27 lines)
        outdated.mdx (35 lines)
        pin.mdx (36 lines)
        regen.mdx (28 lines)
        run.mdx (32 lines)
        setup.mdx (61 lines)
        status.mdx (38 lines)
        unalias.mdx (26 lines)
        uninstall.mdx (19 lines)
        unpin.mdx (30 lines)
        upgrade.mdx (32 lines)
        use.mdx (21 lines)
        versions.mdx (47 lines)
      config.mdx (604 lines)
      detection.mdx (79 lines)
      faq.mdx (123 lines)
      index.mdx (77 lines)
      install.mdx (126 lines)
      non-wasm-plugin.mdx (301 lines)
      plugins.mdx (33 lines)
      tool-spec.mdx (167 lines)
      tools.mdx (25 lines)
      wasm-plugin.mdx (671 lines)
      workflows.mdx (146 lines)
    cheat-sheet.mdx (231 lines)
    comparison.mdx (247 lines)
    create-project.mdx (173 lines)
    create-task.mdx (260 lines)
    faq.mdx (238 lines)
    install.mdx (149 lines)
    intro.mdx (154 lines)
    migrate-to-moon.mdx (75 lines)
    run-task.mdx (132 lines)
    setup-toolchain.mdx (129 lines)
    setup-workspace.mdx (81 lines)
    terminology.md (43 lines)
  src/
    components/
      Docs/
        ActionGraph.tsx (14 lines)
        HeaderLabel.tsx (5 lines)
        HeadingApiLink.tsx (13 lines)
        Image.tsx (6 lines)
        LangGraph.tsx (24 lines)
        ProjectGraph.tsx (14 lines)
        RequiredLabel.tsx (7 lines)
        TaskGraph.tsx (4 lines)
        TomlLink.tsx (8 lines)
        VersionLabel.tsx (10 lines)
        WasmLink.tsx (8 lines)
      Forms/
        RemoteCacheBeta.tsx (13 lines)
      Home/
        CTA.tsx (10 lines)
        ProductSection.tsx (28 lines)
        UsedBy.tsx (11 lines)
      Products/
        Moon/
          Hero.tsx (14 lines)
          HeroTerminal.tsx (23 lines)
        Moonbase/
          Hero.tsx (24 lines)
          Pricing.tsx (55 lines)
          Screenshots.tsx (1 lines)
        Proto/
          Hero.tsx (16 lines)
          HeroTerminal.tsx (3 lines)
          ToolCard.tsx (16 lines)
          ToolCards.tsx (11 lines)
          ToolsGrid.tsx (23 lines)
        AdditionalFeatures.tsx (7 lines)
        Features.tsx (25 lines)
        HeroIcon.tsx (9 lines)
      AddDepsTabs.tsx (26 lines)
      Columns.tsx (6 lines)
      ComparisonColumn.tsx (6 lines)
      ComparisonTable.tsx (16 lines)
      CreateDepTabs.tsx (14 lines)
      FeatureStatus.tsx (12 lines)
      Image.tsx (17 lines)
      LangPartials.tsx (10 lines)
      LangSelector.tsx (18 lines)
      NextSteps.tsx (6 lines)
      NonWasmTabs.tsx (15 lines)
      TwoColumn.tsx (9 lines)
    css/
      custom.css (59 lines)
      theme.css (5 lines)
    data/
      proto-tools.tsx (40 lines)
    js/
      darkModeSyncer.ts (13 lines)
    pages/
      index.tsx (39 lines)
      moon.tsx (14 lines)
      moonbase.tsx (54 lines)
      proto.tsx (10 lines)
    theme/
      Footer/
        Layout/
          ContactForm.tsx (10 lines)
          index.tsx (12 lines)
        Links/
          MultiColumn.tsx (7 lines)
      A.tsx (3 lines)
      DocBreadcrumbs.tsx (21 lines)
      PaginatorNavLink.tsx (8 lines)
      prism-include-languages.js (16 lines)
    ui/
      iconography/
        Icon.tsx (12 lines)
        ProductIcon.tsx (9 lines)
      typography/
        Heading.tsx (26 lines)
        Label.tsx (16 lines)
        Link.tsx (13 lines)
        Text.tsx (60 lines)
        types.ts (68 lines)
      Button.tsx (23 lines)
    utils/
      renderGraph.ts (28 lines)
  static/
    brand/
      moon/
        icon-vector.svg (4 lines)
        icon.svg (37 lines)
        logo-vector.svg (5 lines)
        logo.svg (54 lines)
        text-vector.svg (3 lines)
      moonbase/
        icon-vector.svg (4 lines)
        icon.svg (35 lines)
        logo-vector.svg (4 lines)
        logo.svg (36 lines)
        text-vector.svg (3 lines)
      moonrepo/
        text-vector.svg (3 lines)
      original/
        icon-vector.svg (4 lines)
        icon.svg (37 lines)
        logo-vector.svg (5 lines)
        logo.svg (38 lines)
      proto/
        icon-vector.svg (3 lines)
        icon.svg (24 lines)
        logo-vector.svg (10 lines)
        logo.svg (20 lines)
        text-vector.svg (3 lines)
    brands/
      depot.svg (7 lines)
      gallery.svg (11 lines)
    img/
      tools/
        bun.svg (9 lines)
        cmake.svg (5 lines)
        deno.svg (13 lines)
        dotnet.svg (6 lines)
        go.svg (12 lines)
        jira.svg (5 lines)
        moon.svg (4 lines)
        node.svg (9 lines)
        python.svg (9 lines)
        ruby.svg (3 lines)
        rust.svg (10 lines)
        timoni.svg (1 lines)
        traefik.svg (5 lines)
      backed-by-yc.svg (30 lines)
      favicon.svg (18 lines)
      logo-hero.svg (3 lines)
      logo-yc.svg (42 lines)
      logo.svg (3 lines)
    install/
      moon.ps1 (69 lines)
      moon.sh (94 lines)
      proto.ps1 (102 lines)
      proto.sh (160 lines)
    schemas/
      v1/
        project.json (1942 lines)
        tasks.json (1148 lines)
        template-frontmatter.json (32 lines)
        template.json (567 lines)
        toolchain.json (1021 lines)
        workspace.json (1041 lines)
      v2/
        extensions.json (77 lines)
        project.json (1952 lines)
        tasks.json (1666 lines)
        template-frontmatter.json (34 lines)
        template.json (575 lines)
        toolchains.json (173 lines)
        workspace.json (1041 lines)
      project.json (1942 lines)
      tasks.json (1148 lines)
      template-frontmatter.json (32 lines)
      template.json (567 lines)
      toolchain.json (1021 lines)
      workspace.json (1041 lines)
    .nojekyll (0 lines)
    CNAME (1 lines)
  .eslintrc.cjs (5 lines)
  .gitignore (20 lines)
  babel.config.js (0 lines)
  docusaurus.config.ts (54 lines)
  moon.yml (45 lines)
  package.json (59 lines)
  prism.config.ts (4 lines)
  sidebars.ts (2 lines)
  tailwind.config.js (7 lines)
  tsconfig.json (36 lines)
.dockerignore (47 lines)
.gitattributes (1 lines)
.gitignore (52 lines)
.prettierignore (23 lines)
.prototools (13 lines)
.yarnrc.yml (27 lines)
babel.config.js (0 lines)
Cargo.toml (115 lines)
CHANGELOG.md (175 lines)
clippy.toml (1 lines)
CODE_OF_CONDUCT.md (68 lines)
CONTRIBUTING.md (163 lines)
depot.json (1 lines)
dist-workspace.toml (29 lines)
eslint.config.mjs (1 lines)
justfile (86 lines)
LICENSE (18 lines)
package.json (32 lines)
prettier.config.js (0 lines)
README.md (77 lines)
rust-toolchain.toml (3 lines)
rustfmt.toml (1 lines)
tailwind.config.js (25 lines)
tsconfig.eslint.json (26 lines)
tsconfig.json (21 lines)
tsconfig.options.json (3 lines)
```