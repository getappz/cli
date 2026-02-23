# DDEV Troubleshooting

Common issues when using DDEV with appz-cli, especially on WSL2.

## Container Has No Internet Access (DNS Resolution Failed)

**Symptoms:** `Could not resolve host: github.com`, `install_nvm.sh` fails, `wp core install` or Composer downloads fail.

**Cause:** Docker containers inherit DNS from the host. On WSL2, systemd-resolved (`127.0.0.53`) or network configs often don't work inside containers.

### Fix 1: Docker Desktop — Add DNS to daemon

1. Open **Docker Desktop** → **Settings** → **Docker Engine**
2. Add `"dns"` to the JSON config (merge with existing keys):

   ```json
   {
     "dns": ["8.8.8.8", "1.1.1.1"]
   }
   ```

3. **Apply & Restart**

### Fix 2: Upgrade WSL (recommended for WSL2)

WSL 2.2.1+ includes DNS tunneling, which resolves many Docker DNS issues:

```powershell
wsl --shutdown
wsl --upgrade
```

Verify: `wsl --version` (should show 2.2.1 or higher).

Optional: enable DNS tunneling in `C:\Users\<You>\.wslconfig`:

```ini
[wsl2]
dnsTunneling=true
```

Then `wsl --shutdown` and reopen WSL.

### Fix 3: WSL2 resolv.conf (if not using Docker Desktop)

If Docker runs natively in WSL2:

1. Edit `/etc/wsl.conf`:

   ```ini
   [network]
   generateResolvConf = false
   ```

2. Create `/etc/resolv.conf`:

   ```
   nameserver 8.8.8.8
   nameserver 1.1.1.1
   ```

3. Make it immutable: `sudo chattr +i /etc/resolv.conf`
4. `wsl --shutdown` and reopen

### Verify

After applying a fix:

```bash
ddev exec curl -sSf -o /dev/null --connect-timeout 5 https://github.com && echo "OK" || echo "FAILED"
```

## mkcert Not Installed (HTTPS Warning)

**Symptoms:** "mkcert may not be properly installed, we suggest installing it for trusted https support".

**Cause:** DDEV uses mkcert for trusted local HTTPS. Without it, DDEV falls back to HTTP or self-signed certs (browser warnings).

### Fix

Install mkcert and the system trust store, then install the CA:

**Linux (WSL2):**

```bash
# Install mkcert
sudo apt install libnss3-tools
curl -JLO "https://dl.filippo.io/mkcert/latest?for=linux/amd64"
chmod +x mkcert-v*-linux-amd64 && sudo mv mkcert-*-linux-amd64 /usr/local/bin/mkcert

# Or via package manager
# Ubuntu: sudo apt install mkcert
# brew: brew install mkcert nss

# Install the CA (one-time)
mkcert -install
```

**macOS:** `brew install mkcert nss` then `mkcert -install`

**Windows:** `choco install -y mkcert` then `mkcert -install`

Then restart DDEV: `ddev stop` → `ddev start`

## NVM Warning / install_nvm.sh Failed

**Symptoms:** "Warning: command 'install_nvm.sh' run as 'avihs' failed", "Failed to clone nvm repo".

**Cause:** DDEV no longer includes NVM by default (v1.25+). If you see this, either:

- An older DDEV image still runs nvm setup (and fails when the container has no internet), or
- The `ddev-nvm` add-on is installed and tries to clone nvm at startup.

**Fix:** You likely don't need NVM if you use mise for Node outside the container:

1. Remove the NVM add-on if present:
   ```bash
   ddev add-on remove nvm
   ddev restart
   ```

2. Use `nodejs_version` in DDEV config for in-container Node (no nvm):
   ```bash
   ddev config --nodejs-version=20
   ```
   Or `--nodejs-version=auto` to read from `.nvmrc` / `package.json` engines.

3. For WordPress/PHP-only projects, Node inside the container is usually unnecessary; use mise on the host.

## Simply Static (WordPress static export)

**Requirements:** [Simply Static Pro](https://simplystatic.com/) — WP-CLI support is Pro only. The free version has no `wp simply-static` command.

**Usage with appz build:** When you run `appz build` in a WordPress DDEV project, appz runs `ddev exec wp simply-static run` to export the site to static HTML.

**Setup:**

1. Install Simply Static Pro and activate your license.
2. In WordPress Admin → Simply Static → Settings → Deployment:
   - Choose **Local Directory** as the deployment method.
   - Set **Target Directory** to the path where static files should go. For appz compatibility, use a path that resolves to `<project>/simply-static-output` (e.g. `/var/www/html/simply-static-output` inside the DDEV container, which maps to `simply-static-output` in your project root).
3. Run `appz build` — the export runs and `.appz/output/` is populated from your configured output directory.

**Verify WP-CLI:** `ddev exec wp simply-static` — if you see "Unknown command", you need Simply Static Pro.

## Reference

- [DDEV Config Options](https://docs.ddev.com/en/stable/users/configuration/config/)
- [DDEV Trusted HTTPS (mkcert)](https://ddev.com/blog/ddev-local-trusted-https-certificates/)
- [WSL2 DNS Issues](https://github.com/microsoft/WSL/issues/8365)
