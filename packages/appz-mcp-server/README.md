# appz-mcp-server

MCP (Model Context Protocol) server launcher for [appz](https://github.com/appz-dev/appz-cli). Exposes appz CLI commands and a sandboxed shell to AI assistants (Cursor, Claude Desktop, etc.).

**Requires appz to be installed** — this package is a thin wrapper that runs `appz mcp-server`.

## Install appz

```bash
cargo install appz
# or install from a release binary
```

## Usage

```bash
npx -y appz-mcp-server
```

Or install globally:

```bash
npm install -g appz-mcp-server
appz-mcp-server
```

## Cursor / Claude Configuration

Add to your MCP config:

```json
{
  "mcpServers": {
    "appz": {
      "command": "npx",
      "args": ["-y", "appz-mcp-server"]
    }
  }
}
```

Or use the appz binary directly (no npm package needed):

```json
{
  "mcpServers": {
    "appz": {
      "command": "appz",
      "args": ["mcp-server"]
    }
  }
}
```

## Tools

- **init**, **build**, **dev**, **deploy** — project commands
- **run**, **plan**, **ls** — auth-required (run `appz login` first)
- **skills_add**, **skills_list**, **skills_remove** — Agent Skills
- **shell** — sandboxed shell (project-root scoped, mise environment)
