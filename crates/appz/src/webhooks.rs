//! `appz webhooks`: manage appz.dev webhook endpoints and verify signatures.

use clap::{Args, Subcommand};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

use crate::{assume_yes, error, info, intro, is_tty, outro, success, warning};
use appz_deploy::webhooks::{Webhook, WebhookDelivery, WebhookQuery};

#[derive(Args)]
pub struct WebhooksArgs {
    #[command(subcommand)]
    command: WebhooksCmd,
    /// Provider slug (default: detects from project or config)
    #[arg(long, global = true)]
    target: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum WebhooksCmd {
    /// List webhook endpoints, newest first
    #[command(alias = "list")]
    Ls {
        /// Maximum webhooks to return (1-100)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: Option<u32>,
        /// Cursor from a previous page
        #[arg(long)]
        next: Option<String>,
    },
    /// Register an HTTPS webhook; the signing secret is shown once
    #[command(alias = "create")]
    Add {
        /// HTTPS endpoint that receives events
        url: String,
        /// Event to subscribe to (repeatable): deployment.created, deployment.ready,
        /// deployment.error, deployment.canceled, deployment.promoted,
        /// project.created, project.deleted, project.restored
        #[arg(long = "event", short = 'e', value_name = "EVENT")]
        events: Vec<String>,
        /// Only send events for this project (prj_ id)
        #[arg(long, short = 'p', value_name = "PROJECT_ID")]
        project: Option<String>,
    },
    /// Show a webhook (never includes its secret)
    #[command(alias = "get")]
    Inspect { id: String },
    /// Delete a webhook and its delivery records
    #[command(alias = "remove", alias = "delete")]
    Rm { id: String },
    /// Show delivery attempts (status, HTTP code, attempt, time)
    Deliveries {
        id: String,
        /// Maximum deliveries to return (1-100)
        #[arg(long, value_parser = clap::value_parser!(u32).range(1..=100))]
        limit: Option<u32>,
        /// Cursor from a previous page
        #[arg(long)]
        next: Option<String>,
    },
    /// Send one signed webhook.test and print the result
    Test { id: String },
    /// Check a webhook signature locally
    Verify {
        /// Signing secret from `webhooks add` (or set APPZ_WEBHOOK_SECRET to keep it out of shell history)
        #[arg(long)]
        secret: Option<String>,
        /// Value of the Appz-Signature header (t=<unix seconds>,v1=<hex>)
        #[arg(long)]
        signature: String,
        /// File holding the exact raw request body
        #[arg(long)]
        body: PathBuf,
        /// Maximum signature age in seconds (0 disables the check)
        #[arg(long, default_value_t = 300)]
        tolerance: u64,
    },
}

fn hmac_sha256(key: &[u8], msg: &[u8]) -> [u8; 32] {
    let mut k = [0u8; 64];
    if key.len() > 64 {
        k[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        k[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(k.map(|b| b ^ 0x36));
    inner.update(msg);
    let mut outer = Sha256::new();
    outer.update(k.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Verify `t=<secs>,v1=<hex HMAC-SHA256 of "<secs>.<body>">`. `now` is unix seconds.
fn verify_signature(
    secret: &str,
    header: &str,
    body: &[u8],
    tolerance: u64,
    now: u64,
) -> Result<(), String> {
    let (mut ts, mut sig) = (None, None);
    for part in header.trim().split(',') {
        match part.split_once('=') {
            Some(("t", v)) => ts = v.parse::<u64>().ok(),
            Some(("v1", v)) => sig = Some(v),
            _ => {}
        }
    }
    let (Some(ts), Some(sig)) = (ts, sig) else {
        return Err("malformed signature: expected t=<unix seconds>,v1=<hex>".into());
    };
    let mut signed = format!("{ts}.").into_bytes();
    signed.extend_from_slice(body);
    let expected = hex(&hmac_sha256(secret.as_bytes(), &signed));
    // Compare every byte so timing does not reveal where the signatures differ.
    let same = expected.len() == sig.len()
        && expected
            .bytes()
            .zip(sig.to_ascii_lowercase().bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;
    if !same {
        return Err("signature does not match the secret and body".into());
    }
    if tolerance > 0 && now.abs_diff(ts) > tolerance {
        return Err(format!("signature is valid but older than {tolerance}s"));
    }
    Ok(())
}

fn print_json<T: serde::Serialize>(v: &T) {
    println!("{}", serde_json::to_string_pretty(v).unwrap());
}

fn fmt_millis(ms: i64) -> String {
    crate::fmt_millis(Some(ms))
}

fn webhook_line(w: &Webhook) -> String {
    let state = if w.disabled_at.is_some() {
        "disabled"
    } else {
        "active"
    };
    format!(
        "{}  {:<8}  {}  [{}]  project:{}  {}",
        w.id,
        state,
        w.url,
        w.events.join(","),
        w.project_id.as_deref().unwrap_or("all"),
        fmt_millis(w.created_at)
    )
}

fn delivery_line(d: &WebhookDelivery) -> String {
    let code = d.response_status.map_or("-".to_string(), |c| c.to_string());
    let retry = d
        .next_attempt_at
        .map_or(String::new(), |t| format!("  retry:{}", fmt_millis(t)));
    format!(
        "{}  {:<8}  {:<20}  http:{:<4}  attempt:{}  {}{retry}",
        d.id,
        d.status,
        d.event,
        code,
        d.attempt,
        fmt_millis(d.created_at)
    )
}

fn confirm_delete(id: &str, url: &str) -> bool {
    if assume_yes() {
        return true;
    }
    if !is_tty() {
        error("confirmation required — pass --yes to delete without prompting");
    }
    cliclack::confirm(format!(
        "Delete webhook {id} ({url})? Deliveries are removed too."
    ))
    .initial_value(false)
    .interact()
    .unwrap_or(false)
}

pub fn run(args: WebhooksArgs, json: bool) {
    if let WebhooksCmd::Verify {
        secret,
        signature,
        body,
        tolerance,
    } = &args.command
    {
        let secret = secret
            .clone()
            .or_else(|| std::env::var("APPZ_WEBHOOK_SECRET").ok())
            .unwrap_or_else(|| error("pass --secret or set APPZ_WEBHOOK_SECRET"));
        let bytes = std::fs::read(body)
            .unwrap_or_else(|e| error(&format!("cannot read {}: {e}", body.display())));
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let result = verify_signature(&secret, signature, &bytes, *tolerance, now);
        if json {
            let err = result.as_ref().err();
            print_json(&serde_json::json!({ "valid": err.is_none(), "error": err }));
        } else if let Err(e) = &result {
            error(e);
        } else {
            success("signature is valid");
        }
        if result.is_err() {
            std::process::exit(1);
        }
        return;
    }

    let (provider, ctx) = appz_deploy_ctx(args.target);
    let fail = |e: appz_deploy::DeployError| -> ! { error(&e.to_string()) };
    match args.command {
        WebhooksCmd::Ls { limit, next } => {
            let query = WebhookQuery {
                limit,
                cursor: next,
            };
            let page = provider
                .list_webhooks(&ctx, &query)
                .unwrap_or_else(|e| fail(e));
            if json {
                return print_json(&page);
            }
            intro("appz webhooks");
            if page.items.is_empty() {
                info(
                    "no webhooks found\nrun `appz webhooks add <https-url> --event deployment.ready`",
                );
            } else {
                let lines: Vec<String> = page.items.iter().map(webhook_line).collect();
                info(&lines.join("\n"));
                crate::print_next_hint(&page.next, "appz webhooks ls");
            }
            outro("done");
        }
        WebhooksCmd::Add {
            url,
            events,
            project,
        } => {
            let hook = provider
                .add_webhook(&ctx, &url, &events, project.as_deref())
                .unwrap_or_else(|e| fail(e));
            if json {
                return print_json(&hook);
            }
            intro("appz webhooks add");
            success(&format!("registered webhook {}", hook.id));
            info(&webhook_line(&hook));
            if let Some(secret) = &hook.secret {
                warning("Signing secret: store it now, it won't be shown again.");
                println!("{secret}");
            }
            outro("done");
        }
        WebhooksCmd::Inspect { id } => {
            let hook = provider.get_webhook(&ctx, &id).unwrap_or_else(|e| fail(e));
            if json {
                return print_json(&hook);
            }
            intro("appz webhooks inspect");
            info(&format!(
                "id:         {}\nurl:        {}\nevents:     {}\nproject:    {}\nstatus:     {}\ncreated:    {}",
                hook.id,
                hook.url,
                hook.events.join(", "),
                hook.project_id.as_deref().unwrap_or("all projects"),
                hook.disabled_at.map_or("active".to_string(), |t| format!(
                    "disabled {}",
                    fmt_millis(t)
                )),
                fmt_millis(hook.created_at)
            ));
            outro("done");
        }
        WebhooksCmd::Rm { id } => {
            let hook = provider.get_webhook(&ctx, &id).unwrap_or_else(|e| fail(e));
            if !confirm_delete(&id, &hook.url) {
                error("cancelled");
            }
            provider
                .remove_webhook(&ctx, &id)
                .unwrap_or_else(|e| fail(e));
            if json {
                print_json(&serde_json::json!({ "status": "removed", "webhook": id }));
            } else {
                intro("appz webhooks rm");
                success(&format!("removed webhook {id}"));
                outro("done");
            }
        }
        WebhooksCmd::Deliveries { id, limit, next } => {
            let query = WebhookQuery {
                limit,
                cursor: next,
            };
            let page = provider
                .list_webhook_deliveries(&ctx, &id, &query)
                .unwrap_or_else(|e| fail(e));
            if json {
                return print_json(&page);
            }
            intro("appz webhooks deliveries");
            if page.items.is_empty() {
                info("no deliveries in the last 7 days");
            } else {
                let lines: Vec<String> = page.items.iter().map(delivery_line).collect();
                info(&lines.join("\n"));
                crate::print_next_hint(&page.next, &format!("appz webhooks deliveries {id}"));
            }
            outro("done");
        }
        WebhooksCmd::Test { id } => {
            let d = provider.test_webhook(&ctx, &id).unwrap_or_else(|e| fail(e));
            if json {
                print_json(&d);
            } else {
                intro("appz webhooks test");
                info(&delivery_line(&d));
                if d.status == "ok" {
                    success("endpoint accepted the test event");
                } else {
                    warning("endpoint did not accept the test event (no automatic retry)");
                }
                outro("done");
            }
            if d.status != "ok" {
                std::process::exit(1);
            }
        }
        WebhooksCmd::Verify { .. } => unreachable!("handled above"),
    }
}

fn appz_deploy_ctx(
    target: Option<String>,
) -> (
    Box<dyn appz_deploy::DeployProvider>,
    appz_deploy::DeployContext,
) {
    let root = crate::resolve_root(Path::new("."));
    crate::deploy::webhook_provider(&root, target).unwrap_or_else(|e| error(&e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use appz_deploy::webhooks::WEBHOOK_EVENTS;

    #[test]
    fn hmac_matches_rfc4231_case_2() {
        let mac = hmac_sha256(b"Jefe", b"what do ya want for nothing?");
        assert_eq!(
            hex(&mac),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn verify_accepts_good_and_rejects_tampered_or_stale() {
        let body = br#"{"id":"evt_1"}"#;
        let ts = 1_700_000_000u64;
        let mut signed = format!("{ts}.").into_bytes();
        signed.extend_from_slice(body);
        let header = format!("t={ts},v1={}", hex(&hmac_sha256(b"whsec_x", &signed)));
        assert!(verify_signature("whsec_x", &header, body, 300, ts + 10).is_ok());
        assert!(verify_signature("whsec_y", &header, body, 300, ts).is_err());
        assert!(verify_signature("whsec_x", &header, b"{}", 300, ts).is_err());
        assert!(verify_signature("whsec_x", &header, body, 300, ts + 301).is_err());
        assert!(verify_signature("whsec_x", &header, body, 0, ts + 99999).is_ok());
        assert!(verify_signature("whsec_x", "garbage", body, 300, ts).is_err());
    }

    #[test]
    fn cli_parses_add_and_rm() {
        use clap::Parser;
        let cli = crate::AppzCli::try_parse_from([
            "appz",
            "webhooks",
            "add",
            "https://x.test/h",
            "--event",
            "deployment.ready",
            "--event",
            "project.created",
            "--project",
            "prj_abcdefghij12",
        ])
        .unwrap();
        let crate::AppzCmd::Webhooks(a) = cli.command else {
            panic!("expected webhooks")
        };
        assert!(matches!(
            a.command,
            WebhooksCmd::Add { events, project: Some(_), .. } if events.len() == 2
        ));
        let cli =
            crate::AppzCli::try_parse_from(["appz", "webhook", "rm", "whk_x", "--yes"]).unwrap();
        assert!(cli.yes);
    }

    #[test]
    fn event_list_matches_spec() {
        assert_eq!(WEBHOOK_EVENTS.len(), 8);
    }
}
