# oasist backend

A minimal Node.js/Express backend that:

1. **Serves the OpenAPI spec** – `GET /openapi.json` returns the OpenAPI 3.0.3 spec describing all 52 Solana JSON-RPC methods.
2. **Proxies RPC requests** – `POST /` forwards JSON-RPC 2.0 requests to the configured Solana cluster and returns the response.

## Requirements

- Node.js ≥ 18

## Quick start

```bash
cd backend
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

### `GET /openapi.json`

Returns the OpenAPI 3.0.3 specification for the Solana JSON-RPC API.

### `POST /`

Proxies a JSON-RPC 2.0 request to the configured Solana cluster.

**Request body** (JSON-RPC 2.0):
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "getBalance",
  "params": ["83astBRguLMdt2h5U1Tpdq5tjFoJ6noeGwaY3mDLVcri"]
}
```

**Response**: the upstream cluster's JSON-RPC response, forwarded verbatim.

## Running tests

```bash
npm test
```
