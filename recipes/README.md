# Hosting Platform Recipes

Pre-built recipe templates for deploying to popular hosting platforms. These recipes provide common deployment workflows using official CLI tools.

## Quick Start

1. Copy the desired recipe to your project root:
   ```bash
   cp recipes/vercel.yaml recipe.yaml
   ```

2. Edit the `config` section in `recipe.yaml` with your platform-specific settings

3. Run deployment tasks:
   ```bash
   saasctl run deploy
   ```

## Static Site Hosts

### Vercel
**File:** `vercel.yaml`

Best for: Next.js, React, Vue, Angular, and other modern frameworks

**Features:**
- Automatic framework detection
- Preview deployments
- Environment variable management
- Custom domain support
- Edge functions support

**Required Config:**
- `build_command`: Build command (e.g., `npm run build`)
- `build_output`: Output directory (optional, auto-detected)

**Usage:**
```bash
cp recipes/vercel.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to production
- `deploy:preview` - Deploy preview
- `preview` - Local preview
- `vercel:env:list` - List environment variables
- `vercel:domains:list` - List domains

---

### Netlify
**File:** `netlify.yaml`

Best for: Static sites, JAMstack apps, serverless functions

**Features:**
- Serverless functions deployment
- Form handling
- Split testing
- Environment variable management

**Required Config:**
- `build_command`: Build command
- `publish_dir`: Output directory

**Usage:**
```bash
cp recipes/netlify.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to production
- `deploy:preview` - Deploy preview
- `netlify:functions:deploy` - Deploy functions
- `netlify:env:list` - List environment variables

---

### Cloudflare Pages
**File:** `cloudflare-pages.yaml`

Best for: Static sites with Cloudflare Workers integration

**Features:**
- Cloudflare Workers integration
- Global CDN
- Preview deployments
- Custom domain support

**Required Config:**
- `account_id`: Cloudflare account ID
- `project_name`: Project name
- `build_command`: Build command
- `build_output_dir`: Output directory

**Usage:**
```bash
cp recipes/cloudflare-pages.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Cloudflare Pages
- `preview` - Local preview
- `cloudflare:worker:deploy` - Deploy Workers

---

### GitHub Pages
**File:** `github-pages.yaml`

Best for: Open source projects, documentation sites

**Features:**
- Free hosting for public repos
- Jekyll support
- Custom domain support
- Automatic HTTPS

**Required Config:**
- `repository`: GitHub repo (format: `owner/repo`)
- `build_command`: Build command
- `build_output_dir`: Output directory

**Usage:**
```bash
cp recipes/github-pages.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy using gh-pages
- `deploy:git` - Deploy using git push
- `github:domain:set` - Set custom domain

---

### Surge.sh
**File:** `surge.yaml`

Best for: Quick static site deployments

**Features:**
- Simple deployment
- Custom domain support
- Free tier available

**Required Config:**
- `domain`: Surge domain (e.g., `myapp.surge.sh`)
- `build_command`: Build command
- `build_output_dir`: Output directory

**Usage:**
```bash
cp recipes/surge.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Surge.sh
- `surge:domain:set` - Set custom domain

---

### Firebase Hosting
**File:** `firebase.yaml`

Best for: Static sites with Firebase integration

**Features:**
- Firebase integration
- Preview channels
- Multiple site support
- Custom domain support

**Required Config:**
- `project_id`: Firebase project ID
- `build_command`: Build command
- `build_output_dir`: Output directory

**Usage:**
```bash
cp recipes/firebase.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Firebase Hosting
- `deploy:preview` - Deploy preview channel
- `firebase:functions:deploy` - Deploy Functions

---

### AWS S3 + CloudFront
**File:** `aws-s3.yaml`

Best for: Enterprise static sites, high traffic

**Features:**
- S3 static hosting
- CloudFront CDN
- Cache invalidation
- Environment-specific buckets

**Required Config:**
- `s3_bucket`: S3 bucket name
- `aws_region`: AWS region
- `build_command`: Build command
- `build_output_dir`: Output directory
- `cloudfront_distribution_id`: CloudFront ID (optional)

**Usage:**
```bash
cp recipes/aws-s3.yaml recipe.yaml
# Edit config section
# Configure AWS credentials: aws configure
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to S3
- `cloudfront:invalidate` - Invalidate CloudFront cache
- `deploy:with-invalidation` - Deploy and invalidate

---

### Azure Static Web Apps
**File:** `azure-static.yaml`

Best for: Static sites with Azure integration

**Features:**
- Azure integration
- API integration support
- Custom domain support
- Environment variables

**Required Config:**
- `app_name`: Static Web App name
- `resource_group`: Azure resource group
- `build_command`: Build command
- `build_output_dir`: Output directory

**Usage:**
```bash
cp recipes/azure-static.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Azure Static Web Apps
- `azure:env:set` - Set environment variables
- `azure:domains:add` - Add custom domain

---

## Dynamic/Full-Stack Hosts

### Railway
**File:** `railway.yaml`

Best for: Full-stack apps, databases, microservices

**Features:**
- Database hosting
- Environment variable management
- Automatic deployments
- Logs and monitoring

**Required Config:**
- `build_command`: Build command
- `start_command`: Start command
- `project_name`: Project name (optional)

**Usage:**
```bash
cp recipes/railway.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Railway
- `railway:env:set` - Set environment variables
- `railway:logs` - View logs

---

### Render
**File:** `render.yaml`

Best for: Full-stack apps, background workers

**Features:**
- Automatic deployments
- Database hosting
- Background workers
- SSL certificates

**Required Config:**
- `service_id`: Render service ID
- `build_command`: Build command
- `start_command`: Start command

**Usage:**
```bash
cp recipes/render.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Render
- `render:env:set` - Set environment variables
- `render:logs` - View logs

---

### Fly.io
**File:** `fly.yaml`

Best for: Global apps, edge computing

**Features:**
- Multi-region deployment
- Edge computing
- Database hosting
- Secrets management

**Required Config:**
- `app_name`: Fly.io app name
- `build_command`: Build command
- `start_command`: Start command

**Usage:**
```bash
cp recipes/fly.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Fly.io
- `fly:secrets:set` - Set secrets
- `fly:scale` - Scale app

---

### Heroku
**File:** `heroku.yaml`

Best for: Traditional full-stack apps

**Features:**
- Addon ecosystem
- Database hosting
- Background workers
- Logs and monitoring

**Required Config:**
- `app_name`: Heroku app name
- `build_command`: Build command
- `start_command`: Start command

**Usage:**
```bash
cp recipes/heroku.yaml recipe.yaml
# Edit config section
saasctl run deploy
```

**Tasks:**
- `build` - Build the project
- `deploy` - Deploy to Heroku (via git push)
- `heroku:env:set` - Set environment variables
- `heroku:addons:create` - Create addons

---

## Platform Comparison

| Platform | Type | Free Tier | Best For | CLI Tool |
|----------|------|-----------|----------|----------|
| Vercel | Static/SSR | Yes | Next.js, React | `vercel` |
| Netlify | Static/Functions | Yes | JAMstack, Functions | `netlify-cli` |
| Cloudflare Pages | Static/Workers | Yes | Global CDN, Workers | `wrangler` |
| GitHub Pages | Static | Yes | Open source, Docs | `gh-pages` |
| Surge.sh | Static | Yes | Quick deploys | `surge` |
| Firebase | Static/Functions | Yes | Firebase integration | `firebase-tools` |
| AWS S3 | Static | Pay-as-you-go | Enterprise, High traffic | `awscli` |
| Azure Static | Static/API | Yes | Azure integration | `azure-cli` |
| Railway | Full-stack | Limited | Databases, Microservices | `railway` |
| Render | Full-stack | Limited | Background workers | `render` |
| Fly.io | Full-stack | Limited | Global apps, Edge | `flyctl` |
| Heroku | Full-stack | Limited | Traditional apps | `heroku` |

## Common Tasks

Most recipes include these common tasks:

- `build` - Build the project
- `deploy` - Deploy to production
- `deploy:preview` - Deploy preview/staging
- `preview` - Preview locally

## Environment Variables

Most platforms require authentication tokens. Set these via:

1. **Environment variables** (recommended):
   ```bash
   export VERCEL_TOKEN=your_token
   export NETLIFY_AUTH_TOKEN=your_token
   ```

2. **Config section** in `recipe.yaml`:
   ```yaml
   config:
     vercel_token: "your_token"
   ```

3. **CLI login** (some platforms):
   ```bash
   vercel login
   netlify login
   ```

## Customization

All recipes can be customized:

1. **Modify build commands**: Update `build_command` in config
2. **Add custom tasks**: Add tasks to the `tasks` section
3. **Change output directories**: Update `build_output_dir` in config
4. **Add dependencies**: Use `depends_on` in tasks

## Troubleshooting

### CLI tools not found
Recipes automatically install CLI tools via `mise`. If installation fails:
- Ensure `mise` is installed: `curl https://mise.run | sh`
- Check tool names match package names

### Authentication errors
- Verify tokens are set correctly
- Run platform-specific login commands
- Check token permissions

### Build failures
- Verify `build_command` matches your project
- Check `build_output_dir` exists after build
- Review platform-specific build requirements

## Contributing

To add a new platform recipe:

1. Create `recipes/platform-name.yaml`
2. Follow the existing recipe format
3. Include common tasks (build, deploy, preview)
4. Add platform-specific tasks
5. Update this README

## License

These recipes are provided as-is for use with saasctl.

