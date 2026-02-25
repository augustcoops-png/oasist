# oasist backend

A Node.js/Express backend that:

1. **Serves the OpenAPI spec** – `GET /openapi.json` returns the OpenAPI 3.0.3 spec describing all 52 Solana JSON-RPC methods.
2. **Proxies RPC requests** – `POST /` forwards JSON-RPC 2.0 requests (single *or* batch) to the configured Solana cluster and returns the response.
3. **Validates methods** – unknown method names are rejected with a `-32601 Method not found` error before any network call is made.
4. **Health probe** – `GET /health` is always available for load-balancer liveness checks.

## Requirements

- Node.js ≥ 18

## Quick start

```bash
cd backend
cp .env.example .env   # adjust as needed
npm install
npm start
```

The server starts on port `3000` by default.

## Configuration

| Environment variable | Default | Description |
|---|---|---|
| `PORT` | `3000` | Port the server listens on |
| `SOLANA_RPC_URL` | `https://api.mainnet-beta.solana.com` | Solana JSON-RPC endpoint to proxy requests to |

Example — run against devnet on port 8080:

```bash
PORT=8080 SOLANA_RPC_URL=https://api.devnet.solana.com npm start
```

## Endpoints

### `GET /health`

Liveness probe. Always returns `200 {"status":"ok"}` while the server is running.

### `GET /openapi.json`

Returns the OpenAPI 3.0.3 specification for the Solana JSON-RPC API.

### `POST /`

Proxies a JSON-RPC 2.0 request to the configured Solana cluster.  
Supports both **single** objects and **batch** arrays per the JSON-RPC 2.0 spec.  
Unknown method names are rejected locally with `-32601` before the request is forwarded.

**Single request:**
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getBalance",
  "params": ["83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri"]
}
```

**Batch request:**
```json
[
  { "jsonrpc": "2.0", "id": 1, "method": "getSlot" },
  { "jsonrpc": "2.0", "id": 2, "method": "getBlockHeight" }
]
```

## Running tests

```bash
npm test
```

## Docker

```bash
# Build
docker build -t oasist-backend .

# Run (mainnet-beta, port 3000)
docker run -p 3000:3000 oasist-backend

# Run (devnet, custom port)
docker run -p 8080:8080 \
  -e PORT=8080 \
  -e SOLANA_RPC_URL=https://api.devnet.solana.com \
  oasist-backend
```
