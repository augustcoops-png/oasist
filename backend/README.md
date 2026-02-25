# oasist backend

A Node.js/Express backend that:

1. **Serves the OpenAPI spec** – `GET /openapi.json` returns the OpenAPI 3.0.3 spec describing all 52 Solana JSON-RPC methods.
2. **Proxies RPC requests** – `POST /` forwards JSON-RPC 2.0 requests (single *or* batch) to the configured Solana cluster and returns the response.
3. **Validates methods** – unknown method names are rejected with a `-32601 Method not found` error before any network call is made.
4. **Health probe** – `GET /health` is always available for load-balancer liveness checks.
5. **Method discovery** – `GET /methods` returns a sorted list of every supported RPC method.
6. **Endpoint pool discovery** – `GET /endpoints` returns the active upstream pool with round-robin + failover across 27 built-in public nodes.
7. **Request logging** – structured `combined` format logs via `morgan` (disabled during testing).
8. **Graceful shutdown** – handles `SIGTERM`/`SIGINT` to drain in-flight requests before exiting.

## Requirements

- Node.js ≥ 18

## Quick start

```bash
cd backend
cp .env.example .env   # adjust as needed
npm install
npm start
```

The server starts on port `3000` by default and uses the built-in pool of 15 public mainnet endpoints automatically — no configuration required.

## Configuration

| Environment variable | Default | Description |
|---|---|---|
| `PORT` | `3000` | Port the server listens on |
| `SOLANA_RPC_URLS` | *(built-in pool)* | Comma-separated list of Solana JSON-RPC endpoints (round-robin + failover) |
| `SOLANA_RPC_URL` | *(ignored if `SOLANA_RPC_URLS` is set)* | Single Solana JSON-RPC endpoint (legacy) |

Priority order: `SOLANA_RPC_URLS` → `SOLANA_RPC_URL` → built-in public pool.

### Examples

Run with a custom two-node pool (devnet, round-robin):
```bash
SOLANA_RPC_URLS=https://api.devnet.solana.com,https://rpc.ankr.com/solana_devnet npm start
```

Run with just the legacy single-URL variable:
```bash
SOLANA_RPC_URL=https://api.devnet.solana.com npm start
```

Run on a custom port (pool unchanged):
```bash
PORT=8080 npm start
```

## Built-in public endpoint pool

When no environment variable is set the proxy uses all 27 of these free,
no-API-key-required mainnet endpoints in round-robin order with automatic
failover:

| # | URL | Provider |
|---|-----|----------|
| 1 | https://api.mainnet-beta.solana.com | Solana Labs (official) |
| 2 | https://rpc.ankr.com/solana | Ankr |
| 3 | https://solana-mainnet.rpc.extrnode.com | Extrnode |
| 4 | https://solana-rpc.publicnode.com | PublicNode |
| 5 | https://solana.drpc.org | dRPC |
| 6 | https://endpoints.omniatech.io/v1/sol/mainnet/public | Omnia Tech |
| 7 | https://1rpc.io/sol | 1RPC (Automata Network) |
| 8 | https://mainnet.rpcpool.com | RPCPool |
| 9 | https://free.rpcpool.com | RPCPool (free tier) |
| 10 | https://solana.blockpi.network/v1/rpc/public | BlockPI |
| 11 | https://api.mainnet.rpcfast.com | RPCFast |
| 12 | https://solana.public-rpc.com | public-rpc.com |
| 13 | https://solana-api.projectserum.com | Project Serum (community) |
| 14 | https://ssc-dao.genesysgo.net | GenesysGo (community) |
| 15 | https://lb.drpc.org/ogrpc?network=solana | Nodies / dRPC LB |
| 16 | https://api.mainnet.solana.melea.xyz | Melea Trust |
| 17 | https://rpc.hellomoon.io/public | Hello Moon |
| 18 | https://solana.api.onfinality.io/public | OnFinality |
| 19 | https://rpc.solanatracker.io/public | Solana Tracker |
| 20 | https://api.metaplex.com | Metaplex (community) |
| 21 | https://mainnet.rpc.fluxinfra.xyz | Flux Infrastructure |
| 22 | https://solana-mainnet.core.chainstack.com/demo | Chainstack (demo) |
| 23 | https://mainnet.solana-rpc.com | solana-rpc.com |
| 24 | https://node1.solana.chain.love | chain.love (community) |
| 25 | https://api.mainnet-beta.solana.com.tri.ton.one | Triton One |
| 26 | https://solana-mainnet.rpc.syndica.io | Syndica |
| 27 | https://mainnet.helius-rpc.com/public | Helius |

### Failover behaviour

For each proxied request the pool tries endpoints in round-robin order.  
If an endpoint is unreachable (network error / connection refused), the proxy
automatically tries the **next** endpoint in the pool, continuing until a
successful response is received or all endpoints have been exhausted (in which
case a `502` error is returned).

### Inspect the active pool at runtime

```bash
curl http://localhost:3000/endpoints
# → { "count": 27, "endpoints": ["https://api.mainnet-beta.solana.com", ...] }
```

## Endpoints

### `GET /health`

Liveness probe. Always returns `200 {"status":"ok"}` while the server is running.

### `GET /methods`

Discovery endpoint. Returns an alphabetically sorted list of all 52 supported Solana JSON-RPC method names.

```json
{
  "methods": ["getAccountInfo", "getBalance", "getBlock", "..."]
}
```

### `GET /endpoints`

Returns the active upstream RPC endpoint pool.

```json
{
  "count": 27,
  "endpoints": [
    "https://api.mainnet-beta.solana.com",
    "https://rpc.ankr.com/solana",
    "..."
  ]
}
```

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

## Linting

```bash
npm run lint        # check
npm run lint:fix    # auto-fix
```

## Docker

### Single container

```bash
# Build
docker build -t oasist-backend .

# Run with the built-in 15-node public pool (default)
docker run -p 3000:3000 oasist-backend

# Run with a custom pool
docker run -p 3000:3000 \
  -e SOLANA_RPC_URLS=https://api.mainnet-beta.solana.com,https://rpc.ankr.com/solana \
  oasist-backend

# Run against devnet only
docker run -p 3000:3000 \
  -e SOLANA_RPC_URLS=https://api.devnet.solana.com,https://rpc.ankr.com/solana_devnet \
  oasist-backend
```

### Docker Compose (recommended for local development)

```bash
cd backend

# Start with the default built-in pool
docker compose up

# Start with a custom pool
SOLANA_RPC_URLS=https://api.mainnet-beta.solana.com,https://rpc.ankr.com/solana docker compose up

# Stop
docker compose down
```
