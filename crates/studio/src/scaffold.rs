//! Scaffold a minimal Vite+React+Tailwind project (same layout as open-lovable setupViteApp).

use miette::{miette, Result};
use starbase_utils::fs;
use std::path::Path;
use tracing::instrument;

const PACKAGE_JSON: &str = r#"{
  "name": "gen-app",
  "version": "1.0.0",
  "type": "module",
  "scripts": {
    "dev": "vite --host",
    "build": "vite build",
    "preview": "vite preview"
  },
  "dependencies": {
    "react": "^18.2.0",
    "react-dom": "^18.2.0"
  },
  "devDependencies": {
    "@vitejs/plugin-react": "^4.0.0",
    "vite": "^4.3.9",
    "tailwindcss": "^3.3.0",
    "postcss": "^8.4.31",
    "autoprefixer": "^10.4.16"
  }
}
"#;

const VITE_CONFIG_JS: &str = r#"import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

export default defineConfig({
  plugins: [react()],
  server: {
    host: '0.0.0.0',
    port: 5173,
    strictPort: true,
  }
})
"#;

const TAILWIND_CONFIG_JS: &str = r#"/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {},
  },
  plugins: [],
}
"#;

const POSTCSS_CONFIG_JS: &str = r#"export default {
  plugins: {
    tailwindcss: {},
    autoprefixer: {},
  },
}
"#;

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>App</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.jsx"></script>
  </body>
</html>
"#;

const MAIN_JSX: &str = r#"import React from 'react'
import ReactDOM from 'react-dom/client'
import App from './App.jsx'
import './index.css'

ReactDOM.createRoot(document.getElementById('root')).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
)
"#;

const APP_JSX: &str = r#"function App() {
  return (
    <div className="min-h-screen bg-gray-900 text-white flex items-center justify-center p-4">
      <div className="text-center max-w-2xl">
        <p className="text-lg text-gray-400">
          Ready. Describe your site and we will generate it.
        </p>
      </div>
    </div>
  )
}

export default App
"#;

const INDEX_CSS: &str = r#"@tailwind base;
@tailwind components;
@tailwind utilities;

body {
  font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
  background-color: rgb(17 24 39);
}
"#;

/// Write the minimal Vite+React+Tailwind template into `output_dir`.
#[instrument(skip_all)]
pub fn scaffold(output_dir: &Path) -> Result<()> {
    fs::create_dir_all(output_dir).map_err(|e| miette!("Failed to create output dir: {}", e))?;

    let src_dir = output_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|e| miette!("Failed to create src dir: {}", e))?;

    fs::write_file(output_dir.join("package.json"), PACKAGE_JSON)
        .map_err(|e| miette!("Failed to write package.json: {}", e))?;
    fs::write_file(output_dir.join("vite.config.js"), VITE_CONFIG_JS)
        .map_err(|e| miette!("Failed to write vite.config.js: {}", e))?;
    fs::write_file(output_dir.join("tailwind.config.js"), TAILWIND_CONFIG_JS)
        .map_err(|e| miette!("Failed to write tailwind.config.js: {}", e))?;
    fs::write_file(output_dir.join("postcss.config.js"), POSTCSS_CONFIG_JS)
        .map_err(|e| miette!("Failed to write postcss.config.js: {}", e))?;
    fs::write_file(output_dir.join("index.html"), INDEX_HTML)
        .map_err(|e| miette!("Failed to write index.html: {}", e))?;
    fs::write_file(src_dir.join("main.jsx"), MAIN_JSX)
        .map_err(|e| miette!("Failed to write main.jsx: {}", e))?;
    fs::write_file(src_dir.join("App.jsx"), APP_JSX)
        .map_err(|e| miette!("Failed to write App.jsx: {}", e))?;
    fs::write_file(src_dir.join("index.css"), INDEX_CSS)
        .map_err(|e| miette!("Failed to write index.css: {}", e))?;

    Ok(())
}
