"use strict";

/**
 * Curated list of public, no-auth-required Solana JSON-RPC endpoints
 * organised by network.  These are hard-coded so the server works
 * out-of-the-box without any configuration.
 *
 * Sources verified from public registries and provider documentation.
 * The pool is intentionally large so that if any single node is
 * rate-limited, overloaded, or temporarily unavailable the proxy can
 * fall back to a healthy alternative automatically.
 */
const PUBLIC_ENDPOINTS = {
  /** Mainnet-Beta — no API key required. */
  mainnet: [
    // Solana Labs — official
    "https://api.mainnet-beta.solana.com",
    // Ankr — free public tier
    "https://rpc.ankr.com/solana",
    // Extrnode — free public tier
    "https://solana-mainnet.rpc.extrnode.com",
    // PublicNode — open access
    "https://solana-rpc.publicnode.com",
    // dRPC — free tier
    "https://solana.drpc.org",
    // Omnia Tech — open public endpoint
    "https://endpoints.omniatech.io/v1/sol/mainnet/public",
    // 1RPC by Automata Network — privacy-preserving public relay
    "https://1rpc.io/sol",
    // RPCPool — free public
    "https://mainnet.rpcpool.com",
    // BlockPI — public endpoint
    "https://solana.blockpi.network/v1/rpc/public",
    // RPCFast — free public
    "https://api.mainnet.rpcfast.com",
    // public-rpc.com
    "https://solana.public-rpc.com",
    // Project Serum (community)
    "https://solana-api.projectserum.com",
    // GenesysGo (community)
    "https://ssc-dao.genesysgo.net",
    // Nodies — free public
    "https://lb.drpc.org/ogrpc?network=solana",
    // Melea Trust — free public
    "https://api.mainnet.solana.melea.xyz",
  ],

  /** Devnet — for development and testing. */
  devnet: [
    // Solana Labs — official devnet
    "https://api.devnet.solana.com",
    // Ankr devnet
    "https://rpc.ankr.com/solana_devnet",
  ],

  /** Testnet — for pre-production testing. */
  testnet: [
    // Solana Labs — official testnet
    "https://api.testnet.solana.com",
  ],
};

/**
 * Parse a comma-separated string of URLs into a trimmed, non-empty array.
 *
 * @param {string} raw - Comma-separated URL string, e.g. from an env var.
 * @returns {string[]}
 */
function parseUrls(raw) {
  if (!raw || typeof raw !== "string") return [];
  return raw
    .split(",")
    .map((s) => s.trim())
    .filter(Boolean);
}

/**
 * A pool of JSON-RPC upstream endpoints with round-robin selection and
 * ordered-fallback support.
 *
 * Round-robin spreads load evenly across all endpoints.  The `ordered()`
 * method enables transparent failover: callers iterate the returned list
 * and stop at the first successful response.
 */
class RpcPool {
  /**
   * @param {string[]} urls - One or more upstream endpoint URLs.
   */
  constructor(urls) {
    if (!Array.isArray(urls) || urls.length === 0) {
      throw new Error("RpcPool requires at least one URL");
    }
    this._urls = [...urls];
    this._index = 0;
  }

  /**
   * Round-robin pick: returns the next URL and advances the internal pointer.
   *
   * @returns {string}
   */
  pick() {
    const url = this._urls[this._index];
    this._index = (this._index + 1) % this._urls.length;
    return url;
  }

  /**
   * Returns **all** URLs in round-robin order starting from the current
   * pointer position, then advances the pointer by one.
   *
   * Intended for failover: iterate the returned list and stop at the first
   * successful upstream response.
   *
   * @returns {string[]}
   */
  ordered() {
    const start = this._index;
    this._index = (this._index + 1) % this._urls.length;
    return [
      ...this._urls.slice(start),
      ...this._urls.slice(0, start),
    ];
  }

  /**
   * Returns a copy of every URL in pool order.
   *
   * @returns {string[]}
   */
  endpoints() {
    return [...this._urls];
  }

  /**
   * Number of endpoints in this pool.
   *
   * @returns {number}
   */
  size() {
    return this._urls.length;
  }
}

module.exports = { RpcPool, PUBLIC_ENDPOINTS, parseUrls };
