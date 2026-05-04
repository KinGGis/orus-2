# Wealthfolio Web Deployment: Vercel Front + Render Back

This setup matches the current state of the repository:

- frontend deployed separately on Vercel
- backend deployed on Render from the existing Dockerfile
- one shared backend for both test and prod frontends
- Preview and Production on Vercel can reuse the same backend and the same environment variables

## Why this shape

The web backend is currently wired to SQLite via `WF_DB_PATH`, not to the in-progress PostgreSQL storage layer. That means two separate Render backends would not naturally share the same Wealthfolio data store.

The frontend also uses relative API paths such as `/api/v1/...` and same-origin cookie auth. Because of that, the shortest safe deployment path is:

- one Render backend
- one Vercel project
- Vercel `Preview` and `Production` both proxy `/api/*` and `/docs/*` to the same Render backend

## Git shape

Recommended short-term branch strategy:

- `main`: production candidate
- `staging/web-test`: validation branch for the first deployment

Example:

```powershell
git checkout -b staging/web-test
git add -A
git commit -m "feat: baseline wealthfolio web deployment"
git push -u origin staging/web-test
```

## Render backend

The repository now includes a Render blueprint at `render.yaml`.

### Recommended Render settings

- Service type: `Web Service`
- Runtime: `Docker`
- Dockerfile: `./Dockerfile`
- Persistent disk mount: `/data`
- Persistent disk size: `10 GB`
- Auto deploy: `off` for the first rollout

### Required environment variables

- `WF_LISTEN_ADDR=0.0.0.0:8080`
- `WF_DB_PATH=/data/wealthfolio.db`
- `WF_STATIC_DIR=dist`
- `WF_SECRET_KEY=<32-byte key or base64-encoded 32-byte key>`
- `WF_AUTH_PASSWORD_HASH=<argon2id PHC string>`
- `WF_CORS_ALLOW_ORIGINS=https://your-preview-domain.vercel.app,https://your-prod-domain.vercel.app`

### Optional environment variables

- `SNAPTRADE_CLIENT_ID`
- `SNAPTRADE_CONSUMER_KEY`
- `CONNECT_API_URL`
- `CONNECT_AUTH_URL`
- `CONNECT_AUTH_PUBLISHABLE_KEY`

### Important auth note

When `WF_AUTH_PASSWORD_HASH` is enabled, wildcard CORS is rejected by the backend. You must use explicit Vercel origins in `WF_CORS_ALLOW_ORIGINS`.

## Vercel frontend

Use the repo root as the Vercel project root.

### Recommended Vercel settings

- Framework preset: `Vite`
- Root directory: repository root
- Install command: `pnpm install --frozen-lockfile`
- Build command: `pnpm --filter frontend exec vite build`
- Output directory: `dist`

### Current build caveat

The repository currently has frontend TypeScript errors in the Orus integration surface, so `pnpm --filter frontend build` is not the right deployment command yet because it runs `tsc` before Vite.

For short-term delivery, deploy with the Vite-only build command above. This path was validated locally with a successful production bundle build.

### API proxying

The frontend expects same-origin API access. Keep that behavior in production by proxying these paths from Vercel to Render:

- `/api/*`
- `/docs/*`

An example config is provided in `vercel.json.example`.

Replace `YOUR-RENDER-BACKEND.onrender.com` with the actual Render URL, then use that content in Vercel project configuration.

## First rollout sequence

1. Push the deployment branch to GitHub.
2. Create the Render backend from the branch and confirm it boots.
3. Set Render env vars and confirm the service responds.
4. Create the Vercel project from the same repo and same branch.
5. Add the Vercel rewrite configuration pointing to Render.
6. Validate login, holdings, activities, reports, and Orus routes from the Vercel preview URL.
7. Merge to `main` only after the preview environment is acceptable.

## What is and is not shared today

Shared today:

- Render-backed Wealthfolio SQLite data for all Vercel frontends pointing to that backend
- Orus Supabase-backed runtime dependencies already used by `orus-integration`

Not shared today:

- Wealthfolio core data across multiple independent Render backends

That limitation disappears only once the web backend uses the PostgreSQL storage integration instead of the SQLite storage path.