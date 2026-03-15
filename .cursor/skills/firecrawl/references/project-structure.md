# Directory Structure

```
.github/
  archive/
    js-sdk.yml (53 lines)
    publish-rust-sdk.yml (42 lines)
    python-sdk.yml (65 lines)
    rust-sdk.yml (56 lines)
  ISSUE_TEMPLATE/
    bug_report.md (36 lines)
    feature_request.md (26 lines)
    self_host_issue.md (40 lines)
  scripts/
    check_version_has_incremented.py (84 lines)
    eval_run.py (7 lines)
    requirements.txt (3 lines)
  workflows/
    deploy-go-service.yaml (40 lines)
    deploy-image-staging.yml (29 lines)
    deploy-image.yml (81 lines)
    deploy-nuq-postgres.yml (31 lines)
    deploy-playwright.yml (81 lines)
    deploy-redis.yml (34 lines)
    eval-prod.yml (36 lines)
    ghcr-clean.yml (18 lines)
    npm-audit.yml (112 lines)
    publish-js-sdk.yml (40 lines)
    publish-python-sdk.yml (50 lines)
    test-js-sdk.yml (44 lines)
    test-rust-sdk.yml (51 lines)
  CODEOWNERS (120 lines)
  dependabot.yml (47 lines)
apps/
  api/
    .husky/
      pre-commit (1 lines)
    native/
      .cargo/
        config.toml (2 lines)
      src/
        document/
          model/
            mod.rs (43 lines)
          providers/
            doc.rs (273 lines)
            docx.rs (577 lines)
            factory.rs (16 lines)
            mod.rs (11 lines)
            odt.rs (432 lines)
            rtf.rs (342 lines)
            xlsx.rs (51 lines)
          renderers/
            html.rs (83 lines)
            mod.rs (1 lines)
          mod.rs (30 lines)
        crawler.rs (393 lines)
        engpicker.rs (122 lines)
        html.rs (504 lines)
        lib.rs (6 lines)
        pdf.rs (17 lines)
        utils.rs (2 lines)
      .editorconfig (15 lines)
      .gitattributes (18 lines)
      .gitignore (120 lines)
      .prettierignore (8 lines)
      .taplo.toml (7 lines)
      .yarnrc.yml (5 lines)
      build.rs (1 lines)
      Cargo.toml (37 lines)
      package.json (70 lines)
      README.md (87 lines)
      rustfmt.toml (1 lines)
      tsconfig.json (14 lines)
      wasi-worker-browser.mjs (7 lines)
    requests/
      v2/
        browser.requests.http (91 lines)
        crawl.requests.http (57 lines)
        map.requests.http (30 lines)
        scrape.requests.http (58 lines)
        search.requests.http (35 lines)
      branding.requests.http (38 lines)
    sharedLibs/
      go-html-to-md/
        .gitignore (3 lines)
        go.mod (20 lines)
        html-to-markdown.go (68 lines)
        README.md (12 lines)
    src/
      __tests__/
        deep-research/
          unit/
            deep-research-redis.test.ts (12 lines)
        e2e_extract/
          index.test.ts (11 lines)
        e2e_full_withAuth/
          index.test.ts (355 lines)
        e2e_map/
          index.test.ts (2 lines)
          v2_map.test.ts (8 lines)
        e2e_noAuth/
          index.test.ts (13 lines)
        e2e_v1_withAuth/
          index.test.ts (102 lines)
        e2e_v1_withAuth_all_params/
          index.test.ts (85 lines)
        e2e_withAuth/
          index.test.ts (116 lines)
        lib/
          branding/
            processor-color.test.ts (69 lines)
          search-query-builder.test.ts (25 lines)
        snips/
          mocks/
            map-query-params.json (51 lines)
            mocking-works-properly.json (107 lines)
          utils/
            collect-mocks.js (0 lines)
          v0/
            lib.ts (25 lines)
            scrape.test.ts (16 lines)
          v1/
            batch-scrape.test.ts (8 lines)
            billing.test.ts (85 lines)
            concurrency.test.ts (6 lines)
            crawl.test.ts (97 lines)
            deep-research.test.ts (7 lines)
            extract.test.ts (8 lines)
            iframe-selectors.test.ts (2 lines)
            json-extract-format.test.ts (20 lines)
            lib.ts (238 lines)
            map.test.ts (8 lines)
            scrape.test.ts (97 lines)
            search.test.ts (4 lines)
            types-validation.test.ts (44 lines)
            webhook.test.ts (134 lines)
            zdr.test.ts (22 lines)
          v2/
            batch-scrape.test.ts (8 lines)
            billing.test.ts (81 lines)
            concurrency.test.ts (6 lines)
            crawl-prompt.test.ts (1 lines)
            crawl.test.ts (121 lines)
            document-converter.test.ts (5 lines)
            iframe-selectors.test.ts (2 lines)
            lib.ts (206 lines)
            map.test.ts (18 lines)
            parsers.test.ts (14 lines)
            scrape-branding.test.ts (25 lines)
            scrape-cache.test.ts (37 lines)
            scrape-formats.test.ts (9 lines)
            scrape-skip-tls.test.ts (9 lines)
            scrape-viewport.test.ts (5 lines)
            scrape.test.ts (79 lines)
            search.test.ts (13 lines)
            system-prompt-rejection.test.ts (8 lines)
            types-validation.test.ts (68 lines)
            webhook.test.ts (134 lines)
            zdr.test.ts (22 lines)
          generateDomainSplits.test.ts (1 lines)
          lib.ts (50 lines)
          metadata-concat.test.ts (2 lines)
          zdr-helpers.ts (43 lines)
      controllers/
        __tests__/
          crawl.test.ts (13 lines)
        v0/
          admin/
            acuc-cache-clear.ts (6 lines)
            cclog.ts (8 lines)
            check-fire-engine.ts (8 lines)
            concurrency-queue-backfill.ts (21 lines)
            crawl-monitor.ts (29 lines)
            create-user.ts (33 lines)
            index-queue-prometheus.ts (8 lines)
            metrics.ts (8 lines)
            precrawl.ts (4 lines)
            redis-health.ts (27 lines)
            rotate-api-key.ts (11 lines)
            validate-api-key.ts (25 lines)
            zdrcleaner.ts (29 lines)
          crawl-cancel.ts (13 lines)
          crawl-status.ts (30 lines)
          crawl.ts (83 lines)
          keyAuth.ts (12 lines)
          liveness.ts (5 lines)
          readiness.ts (5 lines)
          scrape.ts (51 lines)
          search.ts (56 lines)
        v1/
          __tests__/
            urlValidation.test.ts (2 lines)
          batch-scrape.ts (42 lines)
          concurrency-check.ts (13 lines)
          crawl-cancel.ts (12 lines)
          crawl-errors.ts (24 lines)
          crawl-ongoing.ts (14 lines)
          crawl-status-ws.ts (63 lines)
          crawl-status.ts (64 lines)
          crawl.ts (36 lines)
          credit-usage-historical.ts (23 lines)
          credit-usage.ts (19 lines)
          deep-research-status.ts (21 lines)
          deep-research.ts (36 lines)
          extract-status.ts (34 lines)
          extract.ts (50 lines)
          generate-llmstxt-status.ts (11 lines)
          generate-llmstxt.ts (34 lines)
          map.ts (145 lines)
          queue-status.ts (25 lines)
          scrape-status.ts (6 lines)
          scrape.ts (35 lines)
          search.ts (63 lines)
          token-usage-historical.ts (18 lines)
          token-usage.ts (19 lines)
          types.ts (595 lines)
          x402-search.ts (64 lines)
        v2/
          __tests__/
            agent-status.test.ts (10 lines)
          agent-cancel.ts (12 lines)
          agent-status.ts (18 lines)
          agent.ts (19 lines)
          batch-scrape.ts (42 lines)
          browser.ts (196 lines)
          concurrency-check.ts (16 lines)
          crawl-cancel.ts (14 lines)
          crawl-errors.ts (24 lines)
          crawl-ongoing.ts (14 lines)
          crawl-params-preview.ts (54 lines)
          crawl-status-ws.ts (63 lines)
          crawl-status.ts (84 lines)
          crawl.ts (42 lines)
          credit-usage-historical.ts (18 lines)
          credit-usage.ts (19 lines)
          extract-status.ts (36 lines)
          extract.ts (29 lines)
          f-search.ts (142 lines)
          map.ts (35 lines)
          queue-status.ts (26 lines)
          scrape-status.ts (5 lines)
          scrape.ts (47 lines)
          search.ts (27 lines)
          token-usage-historical.ts (18 lines)
          token-usage.ts (19 lines)
          types.ts (659 lines)
          x402-search.ts (168 lines)
        auth.ts (120 lines)
      lib/
        __tests__/
          deduplicate-obs-array.test.ts (1 lines)
          html-to-markdown.test.ts (1 lines)
          html-transformer.test.ts (8 lines)
          job-priority.test.ts (24 lines)
          merge-null-val-objs.test.ts (3 lines)
          mix-schemas.test.ts (2 lines)
          spread-schema-objects.test.ts (311 lines)
          transform-array-to-obj.test.ts (1 lines)
          url-utils.test.ts (1 lines)
        branding/
          extractHeaderHtmlChunk.ts (32 lines)
          llm.ts (35 lines)
          logo-selector.ts (103 lines)
          merge.ts (57 lines)
          processor.ts (155 lines)
          prompt.ts (47 lines)
          schema.ts (30 lines)
          transformer.ts (77 lines)
          types.ts (227 lines)
        deep-research/
          deep-research-redis.ts (73 lines)
          deep-research-service.ts (44 lines)
          research-manager.ts (108 lines)
        extract/
          completions/
            analyzeSchemaAndPrompt.ts (27 lines)
            batchExtract.ts (57 lines)
            singleAnswer.ts (58 lines)
          fire-0/
            completions/
              analyzeSchemaAndPrompt-f0.ts (19 lines)
              batchExtract-f0.ts (32 lines)
              checkShouldExtract-f0.ts (21 lines)
              singleAnswer-f0.ts (25 lines)
            helpers/
              deduplicate-objs-array-f0.ts (13 lines)
              dereference-schema-f0.ts (3 lines)
              merge-null-val-objs-f0.ts (54 lines)
              mix-schema-objs-f0.ts (17 lines)
              source-tracker-f0.ts (55 lines)
              spread-schemas-f0.ts (17 lines)
              transform-array-to-obj-f0.ts (33 lines)
            usage/
              llm-cost-f0.ts (21 lines)
            build-document-f0.ts (8 lines)
            build-prompts-f0.ts (23 lines)
            document-scraper-f0.ts (38 lines)
            extraction-service-f0.ts (251 lines)
            llmExtract-f0.ts (113 lines)
            reranker-f0.ts (54 lines)
            url-processor-f0.ts (66 lines)
          helpers/
            __tests__/
              source-tracker.test.ts (18 lines)
            deduplicate-objs-array.ts (11 lines)
            dereference-schema.ts (3 lines)
            merge-null-val-objs.ts (56 lines)
            mix-schema-objs.ts (17 lines)
            source-tracker.ts (55 lines)
            spread-schemas.ts (19 lines)
            transform-array-to-obj.ts (39 lines)
          usage/
            llm-cost.ts (24 lines)
            model-prices.ts (3 lines)
          build-document.ts (8 lines)
          build-prompts.ts (29 lines)
          config.ts (0 lines)
          document-scraper.ts (39 lines)
          extract-redis.ts (78 lines)
          extraction-service.ts (255 lines)
          reranker.ts (99 lines)
          team-id-sync.ts (6 lines)
          url-processor.ts (69 lines)
        generate-llmstxt/
          generate-llmstxt-redis.ts (43 lines)
          generate-llmstxt-service.ts (79 lines)
          generate-llmstxt-supabase.ts (33 lines)
        browser-session-activity.ts (19 lines)
        browser-sessions.ts (96 lines)
        canonical-url.test.ts (1 lines)
        canonical-url.ts (3 lines)
        concurrency-limit.ts (119 lines)
        cost-tracking.ts (13 lines)
        crawl-redis.test.ts (1 lines)
        crawl-redis.ts (139 lines)
        custom-error.ts (8 lines)
        default-values.ts (1 lines)
        deployment.ts (4 lines)
        engpicker.ts (48 lines)
        entities.ts (130 lines)
        error-serde.ts (39 lines)
        error.ts (86 lines)
        format-utils.ts (37 lines)
        gcs-jobs.ts (42 lines)
        gcs-pdf-cache.ts (25 lines)
        generic-ai.ts (41 lines)
        html-to-markdown-client.ts (47 lines)
        html-to-markdown.ts (39 lines)
        job-priority.ts (31 lines)
        logger.ts (8 lines)
        map-cosine.ts (29 lines)
        map-utils.ts (128 lines)
        otel-tracer.ts (40 lines)
        parseApi.ts (9 lines)
        permissions.ts (20 lines)
        permu-refactor.test.ts (10 lines)
        ranker.test.ts (11 lines)
        ranker.ts (30 lines)
        retry-utils.ts (40 lines)
        robots-txt.ts (59 lines)
        sandbox-client.ts (134 lines)
        scrape-billing.ts (29 lines)
        search-index-client.ts (141 lines)
        search-query-builder.ts (67 lines)
        strings.ts (0 lines)
        supabase-jobs.ts (49 lines)
        url-utils.ts (34 lines)
        validate-country.ts (0 lines)
        validateUrl.test.ts (2 lines)
        validateUrl.ts (48 lines)
        withAuth.ts (10 lines)
        x402.ts (28 lines)
      main/
        runWebScraper.ts (58 lines)
      routes/
        admin.ts (21 lines)
        shared.ts (70 lines)
        v0.ts (15 lines)
        v1.ts (133 lines)
        v2.ts (171 lines)
      scraper/
        crawler/
          sitemap.ts (49 lines)
        scrapeURL/
          engines/
            document/
              index.ts (42 lines)
            fetch/
              index.ts (22 lines)
            fire-engine/
              branding-script/
                brand-utils.ts (62 lines)
                buttons.ts (15 lines)
                constants.ts (3 lines)
                css-data.ts (13 lines)
                elements.ts (67 lines)
                helpers.ts (20 lines)
                images.ts (73 lines)
                index.ts (33 lines)
                print-script.js (6 lines)
                svg-utils.ts (17 lines)
              brandingScript.ts (16 lines)
              checkStatus.ts (53 lines)
              delete.ts (15 lines)
              index.ts (97 lines)
              scrape.ts (135 lines)
            index/
              index.ts (62 lines)
            pdf/
              index.ts (73 lines)
            playwright/
              index.ts (14 lines)
            utils/
              downloadFile.ts (36 lines)
              safeFetch.ts (36 lines)
              specialtyHandler.ts (51 lines)
            index.ts (170 lines)
          lib/
            __tests__/
              extractImages.test.ts (1 lines)
              extractLinks.test.ts (1 lines)
              rewriteUrl.test.ts (1 lines)
            abortManager.ts (38 lines)
            cacheableLookup.ts (4 lines)
            extractAttributes.ts (36 lines)
            extractImages.ts (51 lines)
            extractLinks.ts (23 lines)
            extractMetadata.ts (18 lines)
            extractSmartScrape.ts (136 lines)
            fetch.ts (57 lines)
            mock.ts (30 lines)
            removeUnwantedElements.ts (21 lines)
            rewriteUrl.ts (11 lines)
            smartScrape.ts (68 lines)
            urlSpecificParams.ts (41 lines)
          postprocessors/
            index.ts (12 lines)
            youtube.ts (3 lines)
          transformers/
            agent.ts (13 lines)
            diff.ts (26 lines)
            index.ts (93 lines)
            llmExtract.test.ts (25 lines)
            llmExtract.ts (281 lines)
            performAttributes.ts (12 lines)
            removeBase64Images.ts (4 lines)
            sendToSearchIndex.ts (59 lines)
            uploadScreenshot.ts (9 lines)
          .gitignore (1 lines)
          error.ts (110 lines)
          index.ts (275 lines)
          README.md (24 lines)
          retryTracker.ts (26 lines)
          scrapeURL.test.ts (65 lines)
        WebScraper/
          __tests__/
            crawler.test.ts (37 lines)
            dns.test.ts (3 lines)
            utils.test.ts (1 lines)
          utils/
            __tests__/
              engine-forcing.test.ts (2 lines)
              maxDepthUtils.test.ts (1 lines)
            blocklist.ts (25 lines)
            ENGINE_FORCING.md (112 lines)
            engine-forcing.ts (34 lines)
            maxDepthUtils.ts (6 lines)
          crawler.ts (195 lines)
          sitemap.ts (39 lines)
      search/
        v2/
          ddgsearch.ts (33 lines)
          fireEngine-v2.ts (44 lines)
          index.ts (39 lines)
          searxng.ts (25 lines)
        execute.ts (55 lines)
        fireEngine.ts (35 lines)
        index.ts (34 lines)
        scrape.ts (70 lines)
        searxng.ts (25 lines)
        transform.ts (8 lines)
      services/
        alerts/
          slack.ts (9 lines)
        billing/
          auto_charge.ts (111 lines)
          batch_billing.ts (127 lines)
          credit_billing.ts (64 lines)
          issue_credits.ts (8 lines)
          stripe.ts (23 lines)
        idempotency/
          create.ts (5 lines)
          validate.ts (10 lines)
        indexing/
          index-worker.ts (118 lines)
          indexer-queue.ts (32 lines)
        ledger/
          data-schemas.ts (55 lines)
          supabase-ledger.ts (31 lines)
          tracking.ts (35 lines)
        logging/
          log_job.ts (203 lines)
        notification/
          email_notification.ts (90 lines)
          notification_string.ts (6 lines)
          notification-check.ts (5 lines)
        subscription/
          enterprise-check.ts (15 lines)
        webhook/
          config.ts (15 lines)
          delivery.ts (58 lines)
          index.ts (11 lines)
          queue.ts (24 lines)
          schema.ts (5 lines)
          types.ts (121 lines)
        worker/
          crawl-logic.ts (22 lines)
          nuq-prefetch-worker.ts (14 lines)
          nuq-worker.ts (12 lines)
          nuq.ts (305 lines)
          redis.ts (34 lines)
          scrape-worker.ts (170 lines)
          team-semaphore.ts (50 lines)
        ab-test-comparison.ts (27 lines)
        ab-test.ts (28 lines)
        agentLivecastWS.ts (24 lines)
        extract-queue.ts (46 lines)
        extract-worker.ts (45 lines)
        index.ts (170 lines)
        queue-jobs.ts (143 lines)
        queue-service.ts (37 lines)
        queue-worker.ts (67 lines)
        rate-limiter.test.ts (371 lines)
        rate-limiter.ts (16 lines)
        redis.ts (36 lines)
        redlock.ts (19 lines)
        sentry.ts (48 lines)
        supabase.ts (41 lines)
        system-monitor.ts (64 lines)
      types/
        branding.ts (227 lines)
        parse-diff.d.ts (46 lines)
      utils/
        integration.ts (18 lines)
      config.ts (72 lines)
      harness.ts (215 lines)
      index.ts (58 lines)
      natives.ts (4 lines)
      types.ts (173 lines)
    utils/
      find_uncovered_files.sh (30 lines)
      logview.js (7 lines)
      urldump-redis.js (0 lines)
      urldump.js (1 lines)
    .dockerignore (5 lines)
    .gitattributes (2 lines)
    .gitignore (21 lines)
    .prettierrc (11 lines)
    audit-ci.jsonc (5 lines)
    Dockerfile (74 lines)
    jest.config.ts (1 lines)
    knip.config.ts (1 lines)
    openapi-v0.json (924 lines)
    openapi.json (2962 lines)
    package.json (185 lines)
    pnpm-workspace.yaml (4 lines)
    requests.http (145 lines)
    tsconfig.json (35 lines)
    v1-openapi.json (2962 lines)
  go-html-to-md-service/
    .dockerignore (26 lines)
    .gitignore (30 lines)
    converter.go (55 lines)
    docker-compose.yml (19 lines)
    Dockerfile (41 lines)
    go.mod (23 lines)
    handler_test.go (46 lines)
    handler.go (97 lines)
    main.go (55 lines)
    Makefile (86 lines)
    requests.http (223 lines)
  js-sdk/
    firecrawl/
      src/
        __tests__/
          e2e/
            v1/
              index.test.ts (41 lines)
            v2/
              utils/
                idmux.ts (17 lines)
              batch.test.ts (13 lines)
              crawl.test.ts (17 lines)
              extract.test.ts (8 lines)
              map.test.ts (7 lines)
              scrape.test.ts (16 lines)
              search.test.ts (14 lines)
              usage.test.ts (7 lines)
              watcher.test.ts (8 lines)
          unit/
            v1/
              monitor-job-status-retry.test.ts (2 lines)
            v2/
              agent.test.ts (14 lines)
              branding.test.ts (4 lines)
              clientOptions.test.ts (5 lines)
              errorHandler.test.ts (2 lines)
              pagination.test.ts (18 lines)
              scrape.unit.test.ts (4 lines)
              validation.test.ts (12 lines)
              zodSchemaToJson.test.ts (7 lines)
        types/
          node-undici.d.ts (1 lines)
        utils/
          zodSchemaToJson.ts (15 lines)
        v1/
          index.ts (1153 lines)
        v2/
          methods/
            agent.ts (34 lines)
            batch.ts (55 lines)
            browser.ts (39 lines)
            crawl.ts (48 lines)
            extract.ts (36 lines)
            map.ts (7 lines)
            scrape.ts (6 lines)
            search.ts (10 lines)
            usage.ts (15 lines)
          utils/
            errorHandler.ts (24 lines)
            getVersion.ts (3 lines)
            httpClient.ts (36 lines)
            pagination.ts (14 lines)
            validation.ts (8 lines)
          client.ts (314 lines)
          types.ts (725 lines)
          watcher.ts (60 lines)
        index.backup.ts (1061 lines)
        index.ts (31 lines)
      .env.example (5 lines)
      .gitignore (132 lines)
      audit-ci.jsonc (4 lines)
      jest.config.js (1 lines)
      LICENSE (21 lines)
      package.json (69 lines)
      README.md (207 lines)
      tsconfig.json (26 lines)
      tsup.config.ts (3 lines)
    .env.example (2 lines)
    audit-ci.jsonc (4 lines)
    example_pagination.ts (36 lines)
    example_v1.js (4 lines)
    example_v1.ts (11 lines)
    example_watcher.ts (13 lines)
    example.js (22 lines)
    example.ts (40 lines)
    LICENSE (21 lines)
    package.json (31 lines)
    tsconfig.json (72 lines)
  nuq-postgres/
    Dockerfile (21 lines)
    nuq.sql (230 lines)
  playwright-service-ts/
    helpers/
      get_error.ts (2 lines)
    .dockerignore (3 lines)
    .gitignore (1 lines)
    api.ts (40 lines)
    audit-ci.jsonc (4 lines)
    Dockerfile (21 lines)
    package.json (27 lines)
    README.md (47 lines)
    tsconfig.json (110 lines)
  python-sdk/
    firecrawl/
      __tests__/
        e2e/
          v2/
            aio/
              conftest.py (43 lines)
              test_aio_batch_scrape.py (25 lines)
              test_aio_crawl.py (86 lines)
              test_aio_extract.py (13 lines)
              test_aio_map.py (13 lines)
              test_aio_scrape.py (30 lines)
              test_aio_search.py (52 lines)
              test_aio_usage.py (20 lines)
              test_aio_watcher.py (11 lines)
            .env.example (5 lines)
            conftest.py (34 lines)
            test_async.py (34 lines)
            test_batch_scrape.py (53 lines)
            test_crawl.py (117 lines)
            test_extract.py (19 lines)
            test_map.py (19 lines)
            test_scrape.py (69 lines)
            test_search.py (84 lines)
            test_usage.py (18 lines)
            test_watcher.py (25 lines)
        unit/
          v2/
            methods/
              aio/
                test_aio_crawl_params.py (2 lines)
                test_aio_crawl_request_preparation.py (25 lines)
                test_aio_crawl_validation.py (3 lines)
                test_aio_map_request_preparation.py (10 lines)
                test_aio_scrape_request_preparation.py (12 lines)
                test_aio_search_request_preparation.py (19 lines)
                test_batch_request_preparation_async.py (10 lines)
                test_ensure_async.py (53 lines)
              test_agent_request_preparation.py (117 lines)
              test_agent_webhook.py (45 lines)
              test_agent.py (131 lines)
              test_batch_request_preparation.py (35 lines)
              test_branding.py (32 lines)
              test_crawl_params.py (32 lines)
              test_crawl_request_preparation.py (74 lines)
              test_crawl_validation.py (61 lines)
              test_map_request_preparation.py (28 lines)
              test_pagination.py (272 lines)
              test_scrape_request_preparation.py (55 lines)
              test_search_request_preparation.py (55 lines)
              test_search_validation.py (151 lines)
              test_usage_types.py (13 lines)
              test_webhook.py (58 lines)
            utils/
              test_metadata_extras_multivalue.py (10 lines)
              test_metadata_extras.py (44 lines)
              test_recursive_schema.py (429 lines)
              test_validation.py (141 lines)
            watcher/
              test_ws_watcher.py (98 lines)
          test_recursive_schema_v1.py (457 lines)
      v1/
        __init__.py (12 lines)
        client.py (2409 lines)
      v2/
        methods/
          aio/
            __init__.py (1 lines)
            agent.py (39 lines)
            batch.py (116 lines)
            browser.py (80 lines)
            crawl.py (212 lines)
            extract.py (17 lines)
            map.py (28 lines)
            scrape.py (17 lines)
            search.py (70 lines)
            usage.py (28 lines)
          agent.py (40 lines)
          batch.py (283 lines)
          browser.py (84 lines)
          crawl.py (342 lines)
          extract.py (27 lines)
          map.py (37 lines)
          scrape.py (47 lines)
          search.py (125 lines)
          usage.py (27 lines)
        utils/
          __init__.py (5 lines)
          error_handler.py (84 lines)
          get_version.py (5 lines)
          http_client_async.py (15 lines)
          http_client.py (57 lines)
          normalize.py (49 lines)
          validation.py (314 lines)
        __init__.py (1 lines)
        client_async.py (218 lines)
        client.py (508 lines)
        types.py (860 lines)
        watcher_async.py (102 lines)
        watcher.py (108 lines)
      __init__.py (28 lines)
      client.py (84 lines)
      firecrawl.backup.py (2135 lines)
      TODO.md (89 lines)
      types.py (36 lines)
    tests/
      test_agent_integration.py (107 lines)
      test_api_key_handling.py (19 lines)
      test_change_tracking.py (12 lines)
      test_timeout_conversion.py (30 lines)
    .env.example (5 lines)
    .gitignore (6 lines)
    .pylintrc (2 lines)
    example_aio.py (33 lines)
    example_pagination.py (85 lines)
    example_v1.py (156 lines)
    example_v2.py (118 lines)
    example_ws.py (34 lines)
    example.py (29 lines)
    LICENSE (21 lines)
    pyproject.toml (54 lines)
    README.md (213 lines)
    requirements.txt (9 lines)
    setup.py (8 lines)
  redis/
    scripts/
      bump_version.sh (91 lines)
      semver (200 lines)
      version.sh (5 lines)
    .dockerignore (2 lines)
    Dockerfile (6 lines)
    fly.toml (22 lines)
    Procfile (2 lines)
    README.md (48 lines)
    start-redis-server.sh (30 lines)
  rust-sdk/
    examples/
      batch_scrape_example.rs (98 lines)
      cancel_crawl_example.rs (29 lines)
      check_crawl_errors_example.rs (48 lines)
      example.rs (44 lines)
      extract_example.rs (143 lines)
      llmstxt_example.rs (107 lines)
      search_example.rs (104 lines)
      v2_example.rs (139 lines)
    src/
      v2/
        agent.rs (645 lines)
        batch_scrape.rs (507 lines)
        client.rs (277 lines)
        crawl.rs (544 lines)
        map.rs (290 lines)
        mod.rs (74 lines)
        scrape.rs (379 lines)
        search.rs (382 lines)
        types.rs (303 lines)
      batch_scrape.rs (218 lines)
      crawl.rs (395 lines)
      document.rs (55 lines)
      error.rs (25 lines)
      extract.rs (415 lines)
      lib.rs (156 lines)
      llmstxt.rs (243 lines)
      map.rs (46 lines)
      scrape.rs (130 lines)
      search.rs (170 lines)
    tests/
      .env.example (2 lines)
      e2e_with_auth.rs (96 lines)
      v2_e2e.rs (180 lines)
    .gitignore (1 lines)
    Cargo.toml (41 lines)
    CHANGELOG.md (7 lines)
    README.md (166 lines)
  test-site/
    public/
      example.json (6 lines)
    src/
      assets/
        firecrawl-light-logo.svg (3 lines)
        firecrawl-light-wordmark.svg (12 lines)
        firecrawl-logo.svg (3 lines)
        firecrawl-wordmark.svg (12 lines)
      components/
        BaseHead.astro (61 lines)
        Footer.astro (79 lines)
        FormattedDate.astro (17 lines)
        Header.astro (103 lines)
        HeaderLink.astro (24 lines)
      content/
        blog/
          firecrawl-v2-series-a-announcement.md (82 lines)
          introducing-firecrawl-templates.md (51 lines)
          introducing-search-endpoint.md (62 lines)
          launch-week-iii-day-1-introducing-change-tracking.md (179 lines)
          launch-week-iii-day-2-announcing-fire-1.md (70 lines)
          launch-week-iii-day-3-extract-v2.md (77 lines)
          launch-week-iii-day-4-announcing-llmstxt-new.md (36 lines)
          launch-week-iii-day-5-dev-day.md (48 lines)
          launch-week-iii-day-6-firecrawl-mcp.md (33 lines)
          launch-week-iii-day-7-integrations.md (34 lines)
          open-researcher-interleaved-thinking.md (84 lines)
          unicode-post.md (16 lines)
      layouts/
        BlogPost.astro (90 lines)
      pages/
        blog/
          category/
            deep/
              nested/
                path/
                  index.astro (172 lines)
            [...slug].astro (193 lines)
          [...slug].astro (20 lines)
          index.astro (171 lines)
        about.astro (33 lines)
        code-block.astro (116 lines)
        index.astro (42 lines)
        robots.txt.ts (5 lines)
      styles/
        global.css (41 lines)
      consts.ts (0 lines)
      content.config.ts (2 lines)
    .gitignore (23 lines)
    .npmrc (1 lines)
    .prettierrc (20 lines)
    astro.config.mjs (3 lines)
    audit-ci.jsonc (4 lines)
    package.json (29 lines)
    README.md (1 lines)
    tsconfig.json (8 lines)
  test-suite/
    data/
      crawl.json (153 lines)
      scrape.json (118 lines)
    index-benchmark/
      run.ipynb (360 lines)
    load-test-results/
      tests-1-5/
        assets/
          test-run-report.json (4639 lines)
        load-test-1.md (98 lines)
        load-test-2.md (93 lines)
        load-test-3.md (107 lines)
        load-test-4.md (103 lines)
        load-test-5.md (94 lines)
      tests-6-7/
        load-test-6.md (104 lines)
        load-test-7.md (127 lines)
        load-test-8.md (116 lines)
    .env.example (5 lines)
    audit-ci.jsonc (4 lines)
    jest.config.js (0 lines)
    jest.setup.js (0 lines)
    load-test.yml (77 lines)
    package.json (13 lines)
    README.md (59 lines)
  ui/
    ingestion-ui/
      public/
        vite.svg (1 lines)
      src/
        components/
          ui/
            button.tsx (12 lines)
            card.tsx (1 lines)
            checkbox.tsx (3 lines)
            collapsible.tsx (0 lines)
            input.tsx (3 lines)
            label.tsx (3 lines)
            radio-group.tsx (5 lines)
          ingestion.tsx (90 lines)
          ingestionV1.tsx (105 lines)
        lib/
          utils.ts (4 lines)
        App.tsx (5 lines)
        index.css (15 lines)
        main.tsx (3 lines)
        vite-env.d.ts (1 lines)
      .gitignore (24 lines)
      audit-ci.jsonc (4 lines)
      components.json (17 lines)
      eslint.config.js (0 lines)
      index.html (16 lines)
      LICENSE (21 lines)
      package.json (44 lines)
      postcss.config.js (0 lines)
      README.md (65 lines)
      tailwind.config.js (1 lines)
      tsconfig.app.json (33 lines)
      tsconfig.json (17 lines)
      tsconfig.node.json (13 lines)
      vite.config.ts (3 lines)
examples/
  aginews-ai-newsletter/
    README.md (6 lines)
  ai-podcast-generator/
    README.md (7 lines)
  blog-articles/
    amazon-price-tracking/
      notebook.ipynb (1753 lines)
      notebook.md (1237 lines)
    deploying_web_scrapers/
      notebook.ipynb (1541 lines)
      notebook.md (988 lines)
    github-actions-tutorial/
      notebook.ipynb (1630 lines)
      notebook.md (1187 lines)
    mastering-map-endpoint/
      mastering-map-endpoint.ipynb (974 lines)
      mastering-map-endpoint.md (500 lines)
    mastering-scrape-endpoint/
      mastering-scrape-endpoint.ipynb (1895 lines)
      mastering-scrape-endpoint.md (983 lines)
    mastering-the-crawl-endpoint/
      mastering-the-crawl-endpoint.ipynb (1503 lines)
    scheduling_scrapers/
      scripts/
        async_scheduler.py (28 lines)
        bs4_scraper.py (64 lines)
        cron_scraper.py (10 lines)
        firecrawl_scraper.py (35 lines)
        scrape_scheduler.py (1 lines)
      notebook.ipynb (1453 lines)
      notebook.md (900 lines)
  claude_stock_analyzer/
    claude_stock_analyzer.py (83 lines)
  claude-3.7-stock-analyzer/
    claude-3.7-stock-analyzer.py (83 lines)
  claude3.7-web-crawler/
    claude3.7-web-crawler.py (100 lines)
  claude3.7-web-extractor/
    claude-3.7-web-extractor.py (88 lines)
  contradiction_testing/
    web-data-contradiction-testing-using-llms.mdx (78 lines)
  crm_lead_enrichment/
    crm_lead_enrichment.py (49 lines)
  deep-research-apartment-finder/
    .env.example (5 lines)
    apartment_finder.py (140 lines)
    README.md (55 lines)
    requirements.txt (3 lines)
  deepseek-v3-company-researcher/
    .gitignore (50 lines)
    deepseek-v3-extract.py (106 lines)
    README.md (81 lines)
    requirements.txt (5 lines)
  deepseek-v3-crawler/
    .gitignore (50 lines)
    deepseek-v3-crawler.py (64 lines)
    README.md (68 lines)
    requirements.txt (3 lines)
  find_internal_link_opportunites/
    find_internal_link_opportunites.ipynb (509 lines)
  full_example_apps/
    README.md (1 lines)
  gemini-2.0-crawler/
    gemini-2.0-crawler.py (142 lines)
  gemini-2.0-web-extractor/
    gemini-2.0-web-extractor.py (88 lines)
  gemini-2.5-crawler/
    .env.example (6 lines)
    gemini-2.5-crawler.py (140 lines)
    README.md (89 lines)
    requirements.txt (5 lines)
  gemini-2.5-screenshot-editor/
    .env.example (11 lines)
    cli.py (278 lines)
    README.md (419 lines)
    requirements.txt (5 lines)
  gemini-2.5-web-extractor/
    .env.example (8 lines)
    .gitignore (34 lines)
    gemini-2.5-web-extractor.py (88 lines)
    README.md (85 lines)
    requirements.txt (4 lines)
  gemini-github-analyzer/
    gemini-github-analyzer.py (112 lines)
  gpt-4.1-company-researcher/
    .env.example (4 lines)
    .gitignore (111 lines)
    gpt-4.1-company-researcher.py (174 lines)
    README.md (65 lines)
    requirements.txt (5 lines)
  gpt-4.1-web-crawler/
    .env.example (5 lines)
    .gitignore (38 lines)
    gpt-4.1-web-crawler.py (96 lines)
    README.md (82 lines)
    requirements.txt (3 lines)
  gpt-4.5-web-crawler/
    gpt-4.5-crawler.py (96 lines)
  grok_web_crawler/
    grok_web_crawler.py (65 lines)
  groq_web_crawler/
    groq_website_analyzer.py (119 lines)
    requirements.txt (3 lines)
  hacker_news_scraper/
    bs4_scraper.py (64 lines)
    firecrawl_scraper.py (35 lines)
    requirements.txt (6 lines)
  haiku_web_crawler/
    haiku_web_crawler.py (63 lines)
  internal_link_assistant/
    internal_link_assistant.py (50 lines)
  job-resource-analyzer/
    job-resources-analyzer.py (113 lines)
  kubernetes/
    cluster-install/
      api.yaml (72 lines)
      configmap.yaml (15 lines)
      nuq-postgres.yaml (53 lines)
      nuq-worker.yaml (58 lines)
      playwright-service.yaml (53 lines)
      README.md (39 lines)
      redis.yaml (45 lines)
      secret.yaml (15 lines)
      worker.yaml (49 lines)
    firecrawl-helm/
      overlays/
        dev/
          values.yaml (1 lines)
        prod/
          values.yaml (1 lines)
      templates/
        _helpers.tpl (18 lines)
        configmap.yaml (14 lines)
        deployment.yaml (53 lines)
        nuq-postgres-deployment.yaml (71 lines)
        nuq-worker-deployment.yaml (67 lines)
        playwright-configmap.yaml (6 lines)
        playwright-deployment.yaml (47 lines)
        playwright-service.yaml (12 lines)
        redis-deployment.yaml (35 lines)
        redis-service.yaml (12 lines)
        secret.yaml (16 lines)
        service.yaml (12 lines)
        worker-deployment.yaml (33 lines)
      .helmignore (23 lines)
      Chart.yaml (5 lines)
      README.md (51 lines)
      values.yaml (85 lines)
  llama-4-maverick-web-crawler/
    .env.example (5 lines)
    .gitignore (48 lines)
    llama4-maverick-web-crawler.py (100 lines)
    README.md (78 lines)
    requirements.txt (3 lines)
  mistral-small-3.1-crawler/
    mistral-small-3.1-crawler.py (106 lines)
  mistral-small-3.1-extractor/
    mistral-small-3.1-extractor.py (152 lines)
  o1_job_recommender/
    o1_job_recommender.py (55 lines)
  o1_web_crawler/
    o1_web_crawler.py (58 lines)
  o1_web_extractor/
    o1_web_extractor.py (62 lines)
  o3-mini_company_researcher/
    o3-mini_company_researcher.py (83 lines)
  o3-mini_web_crawler/
    o3-mini_web_crawler.py (80 lines)
  o3-mini-deal-finder/
    o3-mini-deal-finder.py (78 lines)
  o3-web-crawler/
    .env.example (3 lines)
    .gitignore (70 lines)
    o3-web-crawler.py (73 lines)
    README.md (59 lines)
    requirements.txt (3 lines)
  o4-mini-web-crawler/
    .env.example (2 lines)
    .gitignore (116 lines)
    o4-mini-web-crawler.py (73 lines)
    README.md (61 lines)
    requirements.txt (3 lines)
  openai_swarm_firecrawl/
    .env.example (2 lines)
    main.py (56 lines)
    README.md (37 lines)
    requirements.txt (2 lines)
  openai_swarm_firecrawl_web_extractor/
    .env.example (3 lines)
    main.py (67 lines)
    requirements.txt (4 lines)
  openai-realtime-firecrawl/
    README.md (7 lines)
  R1_company_researcher/
    r1_company_researcher.py (87 lines)
  R1_web_crawler/
    R1_web_crawler.py (87 lines)
  sales_web_crawler/
    .env.example (3 lines)
    app.py (38 lines)
    requirements.txt (4 lines)
  scrape_and_analyze_airbnb_data_e2b/
    .env.template (8 lines)
    .prettierignore (2 lines)
    airbnb_listings.json (453 lines)
    codeInterpreter.ts (15 lines)
    index.ts (31 lines)
    model.ts (1 lines)
    package.json (26 lines)
    prettier.config.mjs (3 lines)
    README.md (31 lines)
    scraping.ts (33 lines)
  simple_web_data_extraction_with_claude/
    simple_web_data_extraction_with_claude.ipynb (259 lines)
  sonnet_web_crawler/
    sonnet_web_crawler.py (63 lines)
  turning_docs_into_api_specs/
    turning_docs_into_api_specs.py (63 lines)
  visualize_website_topics_e2b/
    claude-visualize-website-topics.ipynb (277 lines)
  web_data_extraction/
    web-data-extraction-using-llms.mdx (92 lines)
  web_data_rag_with_llama3/
    web-data-rag--with-llama3.mdx (91 lines)
  website_qa_with_gemini_caching/
    website_qa_with_gemini_caching.ipynb (166 lines)
    website_qa_with_gemini_flash_caching.ipynb (166 lines)
  attributes-extraction-js-sdk.js (9 lines)
  attributes-extraction-python-sdk.py (19 lines)
.gitattributes (2 lines)
.gitignore (55 lines)
.gitmodules (6 lines)
CLAUDE.md (19 lines)
docker-compose.yaml (165 lines)
LICENSE (680 lines)
README.md (601 lines)
SELF_HOST.md (227 lines)
```