# appz MCP Server

MCP (Model Context Protocol) server that exposes appz CLI commands as tools for AI assistants (Cursor, Claude Desktop, etc.).

## Usage

### Via appz subcommand

```bash
appz mcp
```

### Standalone binary

```bash
appz-mcp-server
```

When run standalone, ensure `appz` is on your PATH so the server can spawn it for tool calls.

## Tools

| Tool | Auth Required | Description |
|------|---------------|--------------|
| init | No | Initialize a new project from a template |
| build | No | Build the project |
| dev | No | Start development server |
| deploy | No | Deploy to a hosting provider |
| run | Yes | Run a task |
| plan | Yes | Show execution plan for a task |
| ls | Yes | List deployments |
| skills_add | No | Add an Agent Skill |
| skills_list | No | List installed skills |
| skills_remove | No | Remove an Agent Skill |
| shell | No | Run a command in the appz sandbox (project-root scoped, mise env) |

## Authentication

Auth-required tools (run, plan, ls, etc.) need pre-authentication:

1. Run `appz login` once — token is stored in `~/.appz/auth.json`
2. Or set `APPZ_API_TOKEN` environment variable when launching the MCP server

## Cursor/Claude Configuration

Add to your MCP config (e.g. Cursor settings):

```json
{
  "mcpServers": {
    "appz": {
      "command": "appz",
      "args": ["mcp"]
    }
  }
}
```

With token via env:

```json
{
  "mcpServers": {
    "appz": {
      "command": "appz",
      "args": ["mcp"],
      "env": { "APPZ_API_TOKEN": "your-token-here" }
    }
  }
}
```
