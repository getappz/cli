# Vercel Source Study — Appz Platform Port

> Study of `appz-ref/vercel` packages for porting deployment architecture to Rust. Reference: [2025-02-20-vercel-alignment-design.md](./2025-02-20-vercel-alignment-design.md)

---

## 1. Deploy Flow

### 1.1 Entry Points

| File | Role |
|------|------|
| `packages/cli/src/commands/deploy/index.ts` | Main deploy handler; routes to init, continue, or default deploy |
| `packages/cli/src/util/deploy/create-deploy.ts` | Wraps `now.create()` with error mapping |
| `packages/cli/src/util/deploy/process-deployment.ts` | Orchestrates `createDeployment` from `@vercel/client` |

### 1.2 Flow Summary

1. **CLI deploy entry** (`deploy/index.ts`):
   - Validates paths, reads `vercel.json` / `vercel.config.*`, ensures link (project)
   - Parses env, meta, target (preview | production), regions
   - Calls `createDeploy()` → `now.create()` → `processDeployment()`

2. **`processDeployment`** (`process-deployment.ts`):
   - Builds `requestBody` (env, build, name, project, meta, gitMetadata, target, projectSettings, etc.)
   - Iterates over `createDeployment(clientOptions, requestBody)` (async generator from `@vercel/client`)
   - Handles events: `file-count`, `file-uploaded`, `created`, `building`, `ready`, `alias-assigned`, `error`

3. **`createDeployment`** (`packages/client/src/create-deployment.ts`):
   - Validates path, token
   - Manual mode: yields to `deploy()` with empty files map (user continues later)
   - Otherwise: `buildFileTree()` → `hashes()` or `tar.pack` (if `--archive=tgz`) → yields `hashes-calculated` → `upload()`

4. **`upload`** (`packages/client/src/upload.ts`):
   - Calls `deploy()` first; if response is `missing_files`, collects `shas` to upload
   - Yields `file-count` (total, missing, uploads)
   - Uploads missing files via `uploadFiles()` to `/v2/files` (POST per file)
   - Calls `deploy()` again after uploads; yields `alias-assigned` when ready

5. **`deploy`** (`packages/client/src/deploy.ts`):
   - `postDeployment()`: POST `/v13/deployments` with `{ ...deploymentOptions, files: preparedFiles }`
   - `prepareFiles()`: converts `FilesMap` to `{ file, sha?, size?, mode }[]` (relative paths)
   - On success, polls `checkDeploymentStatus` until `ready` / `alias-assigned`

6. **`continueDeployment`** (`packages/client/src/continue.ts`):
   - For `deploy continue --id <deployment-id>`: reads `.vercel/output`, builds file tree, hashes
   - POST `/deployments/:id/continue` with `{ files: prepareFiles(...) }`
   - If `missing_files`, uploads via `/v2/files`, then POST continue again
   - Yields `created`, then polls until `alias-assigned`

### 1.3 Request / Response Shapes

**POST `/v13/deployments`** (query: `?teamId=...`):

- **Request body** (from `DeploymentOptions` + `requestBody`):
```json
{
  "version": 2,
  "env": {},
  "build": { "env": {} },
  "public": false,
  "name": "project-name",
  "project": "proj_xxx",
  "meta": {},
  "gitMetadata": {},
  "regions": [],
  "target": "preview",
  "projectSettings": {},
  "source": "cli",
  "files": [
    { "file": "index.html", "sha": "abc123", "size": 1024, "mode": 33188 },
    { "file": "static/asset.js", "sha": "def456", "size": 2048, "mode": 33188 }
  ]
}
```

- **Response** (on success):
```json
{
  "id": "dpl_xxx",
  "url": "xxx.vercel.app",
  "inspectorUrl": "https://vercel.com/...",
  "readyState": "BUILDING" | "READY" | ...,
  "target": "preview" | "production"
}
```

- **Response** (when files needed): `400` with `{ "code": "missing_files", "missing": ["sha1", "sha2", ...] }`

**POST `/v2/files`** (per file):

- **Headers**: `Content-Type: application/octet-stream`, `x-now-digest: <sha>`, `x-now-size: <bytes>`
- **Body**: raw file bytes
- **Response**: `200` on success

**POST `/deployments/:id/continue`** (query: `?teamId=...`):

- **Request body**: `{ "files": [ { "file": "static/index.html", "sha": "...", "size": ..., "mode": ... } ] }`
- **Response**: deployment object, or `{ "code": "missing_files", "missing": [...] }`

### 1.4 File Tree Format

- **`FilesMap`**: `Map<string, DeploymentFile>` where key = SHA1 hex digest
- **`DeploymentFile`**:
  - `names: string[]` — absolute paths (for dedup: same content, multiple paths)
  - `data?: Buffer` — file content (omit for dirs)
  - `mode: number` — file mode (e.g. `0o100644`)

- **`prepareFiles(files, clientOptions)`**:
  - For directory: `fileName = relative(path, name)`
  - For single file / array: `fileName = basename`
  - Returns `{ file, sha?, size?, mode }[]` for API

- **`buildFileTree(path, options, debug)`**:
  - Uses `.vercelignore` / `.nowignore` or prebuilt-specific ignores (only `.vercel/output/**` when prebuilt)
  - `readdir` recursive, filters with ignore
  - Returns `{ fileList: string[], ignoreList: string[] }`

---

## 2. Build Output Format (Build Output API v3)

### 2.1 Location

- **Directory**: `.vercel/output/`
- **Presence**: `config.json` must exist for v3 output

### 2.2 config.json Schema (from fixture)

```json
{
  "version": 3,
  "routes": [
    { "handle": "error" },
    { "status": 404, "src": "^(?!/api).*$", "dest": "/404.html" }
  ],
  "crons": []
}
```

- **version**: integer (3 for v3)
- **routes**: array of route objects (handle, status, src, dest)
- **crons**: array (optional)

### 2.3 static/ Structure

- **Directory**: `.vercel/output/static/`
- Static files (e.g. `index.html`) live directly under `static/`
- Served by filesystem route with optional 404 fallback

### 2.4 builds.json (Legacy, not part of v3 API)

From fixture:
```json
{
  "//": "This file was generated by the `vercel build` command. It is not part of the Build Output API.",
  "target": "preview",
  "argv": [...],
  "builds": [
    {
      "require": "@vercel/static",
      "requirePath": "",
      "apiVersion": 2,
      "src": "**",
      "use": "@vercel/static"
    }
  ]
}
```

- Used internally by `vercel build`; not required for prebuilt deploy
- `target`: `preview` | `production`
- `builds`: array of builder configs

### 2.5 build-output-v3 Helpers

- `getBuildOutputDirectory(path)`: returns path if `config.json` exists, else `undefined`
- `readConfig(path)`: parses `config.json`, returns `{ cache?: string[] }` or undefined
- `createBuildOutput(meta, buildCommand, buildOutputPath, framework)`: returns `{ buildOutputVersion: 3, buildOutputPath }`

---

## 3. Framework Detection

### 3.1 fs-detectors

**Main exports** (`packages/fs-detectors/src/`):

- `detectFramework({ fs, frameworkList, useExperimentalFrameworks })` → `string | null` (slug)
- `detectFrameworkRecord({ fs, frameworkList, useExperimentalFrameworks })` → `Framework | null` (with `detectedVersion`)
- `detectFrameworks(...)` → `Framework[]` (all matches)
- `detectFrameworkVersion(frameworkRecord)` → `string | undefined`

**Detection logic** (`detect-framework.ts`):

1. Filter experimental frameworks unless `useExperimentalFrameworks` or env `VERCEL_USE_EXPERIMENTAL_FRAMEWORKS`
2. For each framework, run `matches(fs, framework)`:
   - Uses `detectors.every` (all must match) and `detectors.some` (one must match)
   - Each detector: `path` (e.g. `package.json`), `matchContent` (regex on file), or `matchPackage` (regex for deps in package.json)
   - `fs.hasPath(path)`, `fs.isFile(path)`, `fs.readFile(path)` from abstract `DetectorFilesystem`
3. `removeSupersededFrameworks`: if framework A has `supersedes: ['B']`, remove B from matches
4. Return first remaining match (slug or full record)

**DetectorFilesystem** (abstract):

- `hasPath(name)`, `_readFile(name)`, `_isFile(name)`, `_readdir(name)`, `_chdir(name)`
- `LocalFileSystemDetector`: implements for local disk

### 3.2 Frameworks (packages/frameworks)

**Framework interface** (from `types.ts`):

- `name`, `slug`, `logo`, `website`, `description`, `sort`, `envPrefix`
- `detectors`: `{ every?: FrameworkDetectionItem[], some?: FrameworkDetectionItem[] }`
- `settings`:
  - `installCommand`: `{ placeholder }` or `{ value, placeholder?, ignorePackageJsonScript? }`
  - `buildCommand`: `{ value, placeholder?, ignorePackageJsonScript? }`
  - `devCommand`: `{ value, placeholder? }`
  - `outputDirectory`: `{ value? }` or `{ placeholder }`
- `getOutputDirName(dirPrefix)` → `Promise<string>` (e.g. `'public'`, `'dist'`)
- `defaultRoutes`?: route rules
- `cachePattern`?: glob for cache
- `supersedes`?: slugs of superseded frameworks
- `experimental`?: boolean

**FrameworkDetectionItem**:

- `path`?: file path (default `package.json` if `matchPackage`)
- `matchContent`?: regex for file content
- `matchPackage`?: package name (regex built for package.json deps)

**Example entries** (from `frameworks.ts`):

- **Next.js**: `detectors.every: [{ matchPackage: 'next' }]`, `buildCommand: 'next build'`, `outputDirectory: placeholder`, `getOutputDirName: () => 'public'`
- **Gatsby**: `detectors.every: [{ matchPackage: 'gatsby' }]`, `buildCommand: 'gatsby build'`, `outputDirectory: { value: 'public' }`, `getOutputDirName: () => 'public'`
- **Blitz.js**: `detectors.some: [{ path: 'blitz.config.js' }, { path: 'blitz.config.ts' }]`, `buildCommand: 'blitz build'`

### 3.3 Detection Flow Summary

1. Create `DetectorFilesystem` (e.g. `LocalFileSystemDetector` for project root)
2. Pass `frameworks` list from `@vercel/frameworks`
3. Call `detectFrameworkRecord({ fs, frameworkList })` → returns best match
4. Use `settings.buildCommand`, `settings.outputDirectory`, `getOutputDirName()` for build

---

## 4. API Endpoints Summary

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/v13/deployments` | POST | Create deployment (with file list); returns deployment or `missing_files` |
| `/v2/files` | POST | Upload single file (content-addressed by SHA) |
| `/deployments/:id/continue` | POST | Continue manual deployment after upload |

---

## 5. Key Types (Rust Port Targets)

- **DeploymentFile**: `{ names: string[], data?: Buffer, mode: number }`
- **FilesMap**: `Map<sha, DeploymentFile>` (content-addressed)
- **PreparedFile**: `{ file: string, sha?, size?, mode }` for API
- **DeploymentOptions**: env, build.env, name, target, meta, projectSettings, gitMetadata, etc.
- **Framework**: name, slug, detectors, settings (installCommand, buildCommand, devCommand, outputDirectory), getOutputDirName
- **Build Output v3**: `.vercel/output/config.json` (version, routes, crons), `static/` directory

---

## 6. References

- Plan: [2025-02-20-vercel-alignment-implementation-plan.md](./2025-02-20-vercel-alignment-implementation-plan.md)
- Design: [2025-02-20-vercel-alignment-design.md](./2025-02-20-vercel-alignment-design.md)
- Source: `appz-ref/vercel/packages/{cli,client,static-build,fs-detectors,frameworks}`
