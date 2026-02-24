"use strict";

const path = require("path");
const express = require("express");
const rateLimit = require("express-rate-limit");

const SPEC_PATH = path.resolve(__dirname, "../docs/static/openapi.json");
const DEFAULT_RPC_URL = process.env.SOLANA_RPC_URL || "https://api.mainnet-beta.solana.com";
const PORT = parseInt(process.env.PORT || "3000", 10);

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
   * GET /openapi.json
   * Serve the OpenAPI 3.0.3 spec that describes the Solana JSON-RPC API.
   */
  app.get("/openapi.json", (_req, res) => {
    res.sendFile(SPEC_PATH);
  });

  /**
   * POST /
   * Proxy a JSON-RPC 2.0 request to the configured Solana cluster and
   * return the cluster's response verbatim.
   */
  app.post("/", async (req, res) => {
    const body = req.body;

    // Basic structural validation: must be a JSON-RPC 2.0 object with a method.
    if (
      !body ||
      body.jsonrpc !== "2.0" ||
      typeof body.method !== "string" ||
      body.method.trim() === ""
    ) {
      return res.status(400).json({
        jsonrpc: "2.0",
        id: body && body.id !== undefined ? body.id : null,
        error: {
          code: -32600,
          message: "Invalid Request: expected a JSON-RPC 2.0 object with a non-empty method field",
        },
      });
    }

    let upstream;
    try {
      upstream = await fetch(rpcUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(body),
      });
    } catch (err) {
      return res.status(502).json({
        jsonrpc: "2.0",
        id: body.id !== undefined ? body.id : null,
        error: {
          code: -32603,
          message: `Upstream error: ${err.message}`,
        },
      });
    }

    const data = await upstream.json();
    return res.status(upstream.status).json(data);
  });

  return app;
}

// Start the server only when this file is run directly (not when it is required by tests).
if (require.main === module) {
  const app = buildApp();
  app.listen(PORT, () => {
    console.log(`oasist backend listening on http://localhost:${PORT}`);
    console.log(`  OpenAPI spec : http://localhost:${PORT}/openapi.json`);
    console.log(`  RPC proxy    : POST http://localhost:${PORT}/  →  ${DEFAULT_RPC_URL}`);
  });
}

module.exports = { buildApp };
