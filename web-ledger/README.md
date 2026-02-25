# Solana Ledger Explorer — Web UI

A browser-based Solana ledger explorer designed to be hosted on an Apache HTTP Server.  
No build step required — it is plain HTML, CSS, and JavaScript.

## Features

| Feature | Description |
|---|---|
| **Live block feed** | Background polling (every 5 s) fetches the latest confirmed slots from the Solana JSON-RPC |
| **Transaction feed** | Auto-refreshing recent-transaction list with status badges |
| **Network stats** | Current slot, epoch, and live TPS from `getRecentPerformanceSamples` |
| **Search** | Search by slot number, transaction signature (88 chars), or account address |
| **Slide-in detail panel** | Click any block or transaction row to see full details (accounts, logs, fees) |
| **Background CLI injection** | Paste a fully-signed, serialized transaction (base64 or base58) and send it directly to the cluster via `sendTransaction` RPC |
| **Network selector** | Switch between mainnet-beta, devnet, testnet, and localnet at runtime |
| **Apache-ready** | `.htaccess` provides CORS headers, compression, caching, and SPA routing |

## File Layout

```
web-ledger/
├── index.html          # Main HTML shell
├── css/
│   └── ledger.css      # Full dark-mode stylesheet
├── js/
│   └── ledger.js       # Background runtime (polling, RPC, rendering, injection)
├── .htaccess           # Apache configuration
└── README.md           # This file
```

## Deployment on Apache HTTP Server

### 1. Enable required Apache modules

```bash
sudo a2enmod rewrite headers deflate expires
sudo systemctl restart apache2
```

### 2. Copy files to the document root

```bash
# Example: deploy to /var/www/html/ledger
sudo cp -r web-ledger/ /var/www/html/ledger
```

### 3. Allow `.htaccess` overrides in your virtual host

In your Apache virtual-host config (e.g. `/etc/apache2/sites-enabled/000-default.conf`):

```apache
<Directory /var/www/html/ledger>
    AllowOverride All
    Require all granted
</Directory>
```

Then reload:

```bash
sudo systemctl reload apache2
```

### 4. Open in your browser

```
http://your-server/ledger/
```

## Pointing to a Custom RPC Endpoint

Edit the `CONFIG.rpcUrl` value at the top of `js/ledger.js`, or use the **network selector** dropdown in the UI at runtime.

For a local validator:

```javascript
rpcUrl: 'http://127.0.0.1:8899',
```

## Injecting Transactions (Background CLI Runtime)

The **Inject Transaction** card at the top of the page lets you submit a raw, fully-signed transaction directly into the ledger:

1. Build and sign your transaction with the Solana SDK or CLI:
   ```bash
   # Example with solana-cli (writes a base64 transaction to stdout)
   solana transfer --from <KEYPAIR> <RECIPIENT> 0.001 --no-wait --output json \
     | jq -r '.rawTransaction'
   ```
2. Paste the base64 (or base58) output into the **Signed Transaction** textarea.
3. Select the matching **Encoding**.
4. Click **Inject Transaction**.

The signature is displayed and can be copied with one click.

## CORS Note

The browser's `fetch()` calls go directly to the Solana JSON-RPC endpoint.  
Public endpoints (`api.mainnet-beta.solana.com`, `api.devnet.solana.com`) already
send permissive CORS headers.  
For a private or self-hosted RPC node, ensure the node is configured to allow
cross-origin requests, or proxy RPC calls through Apache with `mod_proxy`.

## License

Same as the parent repository — see `LICENSE` in the repository root.
