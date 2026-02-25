"use strict";

const path = require("path");
const express = require("express");
const morgan = require("morgan");
const rateLimit = require("express-rate-limit");

const SPEC_PATH = path.resolve(__dirname, "../docs/static/openapi.json");
const DEFAULT_RPC_URL = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
const PORT = parseInt(process.env.PORT || "3000", 10);

// Whitelist of every JSON-RPC method described in the OpenAPI spec.
// Loaded once at startup so POST / can reject unknown method names quickly.
const KNOWN_METHODS = new Set(
  require(SPEC_PATH).components.schemas.JsonRpcRequest.properties.method.enum
);

/**
 * Build and return a configured Express application.
 * Kept as a separate export so tests can import it without starting a live server.
 *
 * @param {string} [rpcUrl] - Solana JSON-RPC endpoint to proxy requests to.
 * @returns {import('express').Application}
 */
function buildApp(rpcUrl = DEFAULT_RPC_URL) {
  const app = express();

  // Parse incoming JSON bodies.
  app.use(express.json());

  // Structured request logging (skip in test env to keep test output clean).
  if (process.env.NODE_ENV !== "test") {
    app.use(morgan("combined"));
  }

  // CORS — allow any origin to call this API (mirrors what public RPC nodes do).
  app.use((_req, res, next) => {
    res.setHeader("Access-Control-Allow-Origin", "*");
    res.setHeader("Access-Control-Allow-Methods", "GET, POST, OPTIONS");
    res.setHeader("Access-Control-Allow-Headers", "Content-Type");
    next();
  });

  // Rate limit: 120 requests per minute per IP across all routes.
  const limiter = rateLimit({
    windowMs: 60 * 1000,
    max: 120,
    standardHeaders: true,
    legacyHeaders: false,
  });
  app.use(limiter);

  // Pre-flight requests.
  app.options("*", (_req, res) => res.sendStatus(204));

  /**
   * GET /health
   * Liveness probe — always returns 200 {"status":"ok"} if the server is up.
   */
  app.get("/health", (_req, res) => {
    res.json({ status: "ok" });
  });

  /**
   * GET /methods
   * Discovery endpoint — returns an alphabetically sorted array of every
   * Solana JSON-RPC method recognised by this proxy.
   */
  app.get("/methods", (_req, res) => {
    res.json({ methods: [...KNOWN_METHODS].sort() });
  });

  /**
   * GET /openapi.json
   * Serve the OpenAPI 3.0.3 spec that describes the Solana JSON-RPC API.
   */
  app.get("/openapi.json", (_req, res) => {
    res.sendFile(SPEC_PATH);
  });

  /**
   * Validate a single JSON-RPC 2.0 request object.
   * Returns an error response object if invalid, otherwise null.
   *
   * @param {object} req
   * @returns {{ jsonrpc: string, id: *, error: { code: number, message: string } } | null}
   */
  function validateRequest(req) {
    const id = req && req.id !== undefined ? req.id : null;
    if (!req || req.jsonrpc !== "2.0" || typeof req.method !== "string" || req.method.trim() === "") {
      return {
        jsonrpc: "2.0",
        id,
        error: {
          code: -32600,
          message: "Invalid Request: expected a JSON-RPC 2.0 object with a non-empty method field",
        },
      };
    }
    if (!KNOWN_METHODS.has(req.method)) {
      return {
        jsonrpc: "2.0",
        id,
        error: {
          code: -32601,
          message: `Method not found: "${req.method}" is not a recognised Solana RPC method`,
        },
      };
    }
    return null;
  }

  /**
   * Forward a single valid JSON-RPC 2.0 object to the upstream cluster.
   *
   * @param {string} rpcUrl
   * @param {object} rpcReq
   * @returns {Promise<{ status: number, body: object }>}
   */
  async function proxyOne(rpcUrl, rpcReq) {
    let upstream;
    try {
      upstream = await fetch(rpcUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(rpcReq),
      });
    } catch (err) {
      return {
        status: 502,
        body: {
          jsonrpc: "2.0",
          id: rpcReq.id !== undefined ? rpcReq.id : null,
          error: { code: -32603, message: `Upstream error: ${err.message}` },
        },
      };
    }
    const body = await upstream.json();
    return { status: upstream.status, body };
  }

  /**
   * POST /
   * Proxy a JSON-RPC 2.0 request (single object OR batch array) to the
   * configured Solana cluster and return the response verbatim.
   *
   * Batch semantics follow JSON-RPC 2.0 §6:
   *   - An empty array returns a 400 error.
   *   - Each element is validated individually; invalid elements get an inline
   *     error response rather than aborting the whole batch.
   *   - All valid requests are forwarded to the upstream in parallel.
   */
  app.post("/", async (req, res) => {
    const body = req.body;

    // ── Batch request ────────────────────────────────────────────────────────
    if (Array.isArray(body)) {
      if (body.length === 0) {
        return res.status(400).json({
          jsonrpc: "2.0",
          id: null,
          error: { code: -32600, message: "Invalid Request: batch array must not be empty" },
        });
      }

      const results = await Promise.all(
        body.map(async (item) => {
          const err = validateRequest(item);
          if (err) return err;
          const { body: responseBody } = await proxyOne(rpcUrl, item);
          return responseBody;
        })
      );
      return res.json(results);
    }

    // ── Single request ───────────────────────────────────────────────────────
    const validationErr = validateRequest(body);
    if (validationErr) {
      return res.status(400).json(validationErr);
    }

    const { status, body: responseBody } = await proxyOne(rpcUrl, body);
    return res.status(status).json(responseBody);
  });

  return app;
}

// Start the server only when this file is run directly (not when it is required by tests).
if (require.main === module) {
  const app = buildApp();
  const server = app.listen(PORT, () => {
    console.log(`oasist backend listening on http://localhost:${PORT}`);
    console.log(`  OpenAPI spec : http://localhost:${PORT}/openapi.json`);
    console.log(`  Methods list : http://localhost:${PORT}/methods`);
    console.log(`  RPC proxy    : POST http://localhost:${PORT}/  →  ${DEFAULT_RPC_URL}`);
  });

  // Graceful shutdown — finish in-flight requests before exiting.
  function shutdown(signal) {
    console.log(`\nReceived ${signal}. Shutting down gracefully…`);
    server.close(() => {
      console.log("Server closed.");
      process.exit(0);
    });
    // Force-exit if draining takes too long.
    setTimeout(() => {
      console.error("Forced exit after timeout.");
      process.exit(1);
    }, 10_000).unref();
  }

  process.on("SIGTERM", () => shutdown("SIGTERM"));
  process.on("SIGINT", () => shutdown("SIGINT"));
}

module.exports = { buildApp };
