# Tractor Web

Browser-based playground for Tractor - XPath for code.

## Development

```bash
npm install
npm run dev
```

Opens at http://localhost:5173

## Build

```bash
npm run build
```

Output goes to `dist/`.

## Routes

- `/` - Homepage with overview and links
- `/playground` - Interactive XPath playground

## Deployment

### Docker / Fly.io

```bash
npm run build
docker build -t tractor-web .
```

The included `fly.toml` is configured for Fly.io deployment:

```bash
fly deploy
```

### Nginx

The `nginx.conf` handles:
- SPA routing (fallback to index.html)
- WASM MIME types
- Asset caching
