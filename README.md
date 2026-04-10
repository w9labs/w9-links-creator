# W9 Links Creator

URL shortener and note drops for w9.nu / w9.se domains.

## Tech Stack

- **Backend**: Rust + Axum + SurrealDB
- **Frontend**: Leptos (Full-stack SSR)
- **ID Generation**: nanoid for short codes

## Features

- Simple URL redirects (`/s/<code>`)
- Note drops (`/n/<code>`)
- Click tracking and analytics
- Custom domain support (w9.nu, w9.se)
- Link expiration and management

## API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | Health check |
| POST | `/api/links/create` | Create short link |
| GET | `/api/links` | List user links |
| DELETE | `/api/links/:id` | Delete link |
| GET | `/api/links/:id/stats` | Link analytics |
| GET | `/s/:code` | Simple redirect |
| GET | `/n/:code` | View note |

## Quick Start

```bash
cargo run --package w9-links-creator-server
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `DATABASE_URL` | SurrealDB connection | `memory` |
| `BASE_URL` | Public base URL | `https://w9.nu` |
| `PORT` | Server port | `8085` |

## Deployment

```bash
docker-compose up -d
```

Access at: `https://links.w9.nu`

## License

GPL v3.0
