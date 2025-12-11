# Dev Server

A high-performance local development server with hot reloading and form data processing capabilities.

## Features

- **Static File Serving**: Serve static files from a configurable directory
- **Hot Reloading**: WebSocket-based live reload for automatic browser refresh on file changes
- **Form Data Processing**: Handle multipart/form-data and application/x-www-form-urlencoded requests
- **SPA Support**: Automatic fallback to index.html for 404 errors (Single Page Application support)
- **SEO-Friendly URLs**: Automatic redirects and URL rewriting for clean URLs
  - `/index.html` → `/`
  - `/demo.html` → `/demo`
  - `/demo/` → `/demo`
  - `/demo/index.html` → `/demo`
  - Smart fallback: tries `demo.html` first, then `demo/index.html`
- **Directory Listing**: Optional directory listing for browsing files
- **CORS Support**: Built-in CORS headers for cross-origin requests

## Usage

### As a Library

```rust
use dev_server::{ServerConfig, DevServer};

let config = ServerConfig {
    address: "127.0.0.1".to_string(),
    port: 3000,
    root_dir: std::path::PathBuf::from("./dist"),
    hot_reload: true,
    enable_forms: false,
    upload_dir: None,
    cors: true,
    directory_listing: false,
    spa_fallback: true,
};

let mut server = DevServer::new(config)?;
server.start().await?;
```

### CLI Command

```bash
# Start server on default port 3000
appz dev-server

# Start on custom port
appz dev-server --port 8080

# Serve from specific directory
appz dev-server --dir ./dist

# Disable hot reload
appz dev-server --no-reload

# Enable form data processing
appz dev-server --enable-forms
```

## Configuration

### ServerConfig

- `address`: Server bind address (default: "127.0.0.1")
- `port`: Server port (default: 3000)
- `root_dir`: Directory to serve files from
- `hot_reload`: Enable WebSocket-based hot reloading (default: true)
- `enable_forms`: Enable form data processing (default: false)
- `upload_dir`: Directory for uploaded files (default: root_dir/uploads)
- `cors`: Enable CORS headers (default: true)
- `directory_listing`: Enable directory listing (default: false)
- `spa_fallback`: Fallback to index.html for 404s (default: true)

## Hot Reloading

When hot reloading is enabled, clients can connect to `ws://localhost:3000/__hot_reload` to receive file change notifications. The server sends JSON messages in the following format:

```json
{
  "type": "reload",
  "event": "modified",
  "path": "src/index.html"
}
```

Events can be:
- `created`: File was created
- `modified`: File was modified
- `deleted`: File was deleted

## SEO-Friendly URLs

The server automatically handles URL rewriting and redirects for SEO-friendly URLs:

### Redirects (301 Permanent Redirect)

- `/index.html` → `/`
- `/demo.html` → `/demo`
- `/demo/` → `/demo` (removes trailing slash)
- `/demo/index.html` → `/demo`

### Fallback Logic

When serving a path like `/demo`, the server tries files in this order:

1. `demo.html` (if exists, serves directly)
2. `demo/index.html` (if exists, serves directly)
3. SPA fallback to `index.html` (if enabled)

This allows you to organize your files flexibly while maintaining clean URLs.

## Form Data Processing

When form processing is enabled, POST requests with `multipart/form-data` or `application/x-www-form-urlencoded` content types are processed:

- **Multipart**: Files are saved to the upload directory, fields are extracted
- **URL-encoded**: Fields are parsed and returned

Response format:
```json
{
  "success": true,
  "fields": {
    "name": "value"
  },
  "files": [
    {
      "name": "file.txt",
      "field": "upload",
      "size": 1024,
      "path": "/path/to/uploads/file.txt"
    }
  ]
}
```

## Architecture

- **Server**: Main HTTP server using Hyper
- **Handlers**: Modular request handlers for static files, forms, and WebSocket
- **Watcher**: File system watcher using `notify` crate with debouncing
- **WebSocket**: Hot reload notifications via WebSocket connections

## Dependencies

- `hyper`: HTTP server
- `tokio`: Async runtime
- `notify`: File system watching
- `multer`: Multipart form parsing
- `tokio-tungstenite`: WebSocket support
- `mime_guess`: MIME type detection

