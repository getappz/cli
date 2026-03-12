use miette::miette;

use crate::shell;
use task::{Task, TaskRegistry};

#[derive(Clone, Default)]
pub struct ArtisanOpts {
    pub min: Option<&'static str>,
    pub max: Option<&'static str>,
    pub skip_if_no_env: bool,
    pub fail_if_no_env: bool,
    pub show_output: bool,
    pub dotenv: Option<String>,
}

fn artisan(command: &'static str, _opts: ArtisanOpts) -> task::AsyncTaskFn {
    task::task_fn_async!(|_ctx: std::sync::Arc<task::Context>| async move {
        let cmd = format!("php artisan {command}");
        shell::run_local(&cmd).map_err(|e| miette!("artisan failed: {:#}", e))
    })
}

pub fn register_laravel(reg: &mut TaskRegistry) {
    // Maintenance
    reg.register(Task::new(
        "artisan:down",
        artisan(
            "down",
            ArtisanOpts {
                show_output: true,
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:up",
        artisan(
            "up",
            ArtisanOpts {
                show_output: true,
                ..Default::default()
            },
        ),
    ));

    // Keys
    reg.register(Task::new(
        "artisan:key:generate",
        artisan("key:generate", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:passport:keys",
        artisan("passport:keys", ArtisanOpts::default()),
    ));

    // DB & migrations
    reg.register(Task::new(
        "artisan:db:seed",
        artisan(
            "db:seed --force",
            ArtisanOpts {
                skip_if_no_env: true,
                show_output: true,
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:migrate",
        artisan(
            "migrate --force",
            ArtisanOpts {
                skip_if_no_env: true,
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:migrate:fresh",
        artisan(
            "migrate:fresh --force",
            ArtisanOpts {
                skip_if_no_env: true,
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:migrate:rollback",
        artisan(
            "migrate:rollback --force",
            ArtisanOpts {
                skip_if_no_env: true,
                show_output: true,
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:migrate:status",
        artisan(
            "migrate:status",
            ArtisanOpts {
                skip_if_no_env: true,
                show_output: true,
                ..Default::default()
            },
        ),
    ));

    // Cache and optimizations
    reg.register(Task::new(
        "artisan:cache:clear",
        artisan("cache:clear", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:config:cache",
        artisan("config:cache", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:config:clear",
        artisan("config:clear", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:optimize",
        artisan("optimize", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:optimize:clear",
        artisan("optimize:clear", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:route:cache",
        artisan("route:cache", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:route:clear",
        artisan("route:clear", ArtisanOpts::default()),
    ));
    reg.register(Task::new(
        "artisan:route:list",
        artisan(
            "route:list",
            ArtisanOpts {
                show_output: true,
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:storage:link",
        artisan(
            "storage:link",
            ArtisanOpts {
                min: Some("5.3"),
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:view:cache",
        artisan(
            "view:cache",
            ArtisanOpts {
                min: Some("5.6"),
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:view:clear",
        artisan("view:clear", ArtisanOpts::default()),
    ));

    // Events (scaffolded version gating)
    reg.register(Task::new(
        "artisan:event:cache",
        artisan(
            "event:cache",
            ArtisanOpts {
                min: Some("5.8.9"),
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:event:clear",
        artisan(
            "event:clear",
            ArtisanOpts {
                min: Some("5.8.9"),
                ..Default::default()
            },
        ),
    ));
    reg.register(Task::new(
        "artisan:event:list",
        artisan(
            "event:list",
            ArtisanOpts {
                min: Some("5.8.9"),
                show_output: true,
                ..Default::default()
            },
        ),
    ));

    // Deploy group (Laravel-specific composition similar to Deployer's recipe)
    // Use namespacing so entrypoints are under `laravel:*`
    let mut laravel = reg.with_namespace("laravel");
    laravel.register(
        Task::new(
            "deploy:vendors",
            task::task_fn_sync!(|_ctx: std::sync::Arc<task::Context>| Ok(())),
        )
        .desc("Install vendors")
        .hidden(),
    );
    laravel.register(
        Task::new(
            "deploy",
            task::task_fn_sync!(|ctx: std::sync::Arc<task::Context>| {
                // Provide Laravel-specific defaults for downstream common tasks
                ctx.set("writable_dirs", "storage/statamic");
                Ok(())
            }),
        )
        .desc("Deploys your project")
        // absolute references to common deploy pipeline
        .depends_on(":deploy:prepare")
        .depends_on("deploy:vendors")
        // artisan tasks remain global `artisan:*`
        .depends_on("artisan:storage:link")
        .depends_on("artisan:config:cache")
        .depends_on("artisan:route:cache")
        .depends_on("artisan:view:cache")
        .depends_on("artisan:event:cache")
        .depends_on("artisan:migrate")
        .depends_on(":deploy:publish"),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use task::{Context, TaskRegistry};

    #[tokio::test]
    async fn artisan_skip_if_no_env() {
        let mut reg = TaskRegistry::new();
        let t = Task::new(
            "artisan:test:skip",
            artisan(
                "list",
                ArtisanOpts {
                    skip_if_no_env: true,
                    dotenv: Some("./path/does/not/exist/.env".to_string()),
                    ..Default::default()
                },
            ),
        );
        reg.register(t);
        let ctx = Arc::new(Context::new());
        // Should not error and not require php
        let res = (reg.get("artisan:test:skip").unwrap().action)(ctx).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn artisan_fail_if_no_env() {
        let mut reg = TaskRegistry::new();
        let t = Task::new(
            "artisan:test:fail",
            artisan(
                "list",
                ArtisanOpts {
                    fail_if_no_env: true,
                    dotenv: Some("./path/does/not/exist/.env".to_string()),
                    ..Default::default()
                },
            ),
        );
        reg.register(t);
        let ctx = Arc::new(Context::new());
        let res = (reg.get("artisan:test:fail").unwrap().action)(ctx).await;
        assert!(res.is_err());
    }
}
