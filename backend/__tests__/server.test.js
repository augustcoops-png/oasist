"use strict";

const request = require("supertest");
const { buildApp } = require("../server");

// ─── GET /openapi.json ────────────────────────────────────────────────────────
describe("GET /openapi.json", () => {
  const app = buildApp();

  it("returns 200 with JSON content-type", async () => {
    const res = await request(app).get("/openapi.json");
    expect(res.status).toBe(200);
    expect(res.headers["content-type"]).toMatch(/application\/json/);
  });

  it("returns a valid OpenAPI 3.0.3 document", async () => {
    const res = await request(app).get("/openapi.json");
    const doc = res.body;
    expect(doc.openapi).toBe("3.0.3");
    expect(doc.info).toBeDefined();
    expect(doc.paths).toBeDefined();
  });

  it("includes all 52 RPC methods in the method enum", async () => {
    const res = await request(app).get("/openapi.json");
    const methods =
      res.body.components.schemas.JsonRpcRequest.properties.method.enum;
    expect(Array.isArray(methods)).toBe(true);
    expect(methods.length).toBe(52);
    expect(methods).toContain("getBalance");
    expect(methods).toContain("sendTransaction");
    expect(methods).toContain("simulateTransaction");
  });

  it("sets CORS header", async () => {
    const res = await request(app).get("/openapi.json");
    expect(res.headers["access-control-allow-origin"]).toBe("*");
  });
});

// ─── GET /health ──────────────────────────────────────────────────────────────
describe("GET /health", () => {
  const app = buildApp();

  it("returns 200 with status ok", async () => {
    const res = await request(app).get("/health");
    expect(res.status).toBe(200);
    expect(res.body).toEqual({ status: "ok" });
  });

  it("sets CORS header", async () => {
    const res = await request(app).get("/health");
    expect(res.headers["access-control-allow-origin"]).toBe("*");
  });
});

// ─── GET /methods ─────────────────────────────────────────────────────────────
describe("GET /methods", () => {
  const app = buildApp();

  it("returns 200 with JSON content-type", async () => {
    const res = await request(app).get("/methods");
    expect(res.status).toBe(200);
    expect(res.headers["content-type"]).toMatch(/application\/json/);
  });

  it("returns an object with a methods array of all 52 methods", async () => {
    const res = await request(app).get("/methods");
    expect(Array.isArray(res.body.methods)).toBe(true);
    expect(res.body.methods.length).toBe(52);
  });

  it("returns methods in alphabetical order", async () => {
    const res = await request(app).get("/methods");
    const methods = res.body.methods;
    const sorted = [...methods].sort();
    expect(methods).toEqual(sorted);
  });

  it("includes known methods", async () => {
    const res = await request(app).get("/methods");
    expect(res.body.methods).toContain("getBalance");
    expect(res.body.methods).toContain("sendTransaction");
    expect(res.body.methods).toContain("getSlot");
  });

  it("sets CORS header", async () => {
    const res = await request(app).get("/methods");
    expect(res.headers["access-control-allow-origin"]).toBe("*");
  });
});

// ─── POST / single request ────────────────────────────────────────────────────
describe("POST / (single request)", () => {
  // Port 19999 is intentionally unreachable so proxy tests don't hit the network.
  const app = buildApp("http://127.0.0.1:19999");

  it("returns 400 for a missing jsonrpc field", async () => {
    const res = await request(app)
      .post("/")
      .send({ id: 1, method: "getBalance" });
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe(-32600);
  });

  it("returns 400 for a missing method field", async () => {
    const res = await request(app)
      .post("/")
      .send({ jsonrpc: "2.0", id: 1 });
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe(-32600);
  });

  it("returns 400 for an empty method string", async () => {
    const res = await request(app)
      .post("/")
      .send({ jsonrpc: "2.0", id: 1, method: "   " });
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe(-32600);
  });

  it("returns 400 with -32601 for an unknown method", async () => {
    const res = await request(app)
      .post("/")
      .send({ jsonrpc: "2.0", id: 2, method: "notARealMethod" });
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe(-32601);
    expect(res.body.id).toBe(2);
  });

  it("returns 502 when upstream is unreachable", async () => {
    const res = await request(app)
      .post("/")
      .send({ jsonrpc: "2.0", id: 1, method: "getBalance", params: ["pubkey"] });
    expect(res.status).toBe(502);
    expect(res.body.error.code).toBe(-32603);
  });

  it("echoes the request id in error responses", async () => {
    const res = await request(app)
      .post("/")
      .send({ id: 42, method: "bad" });
    expect(res.body.id).toBe(42);
  });
});

// ─── POST / batch request ─────────────────────────────────────────────────────
describe("POST / (batch request)", () => {
  const app = buildApp("http://127.0.0.1:19999");

  it("returns 400 for an empty batch array", async () => {
    const res = await request(app).post("/").send([]);
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe(-32600);
  });

  it("returns inline -32601 for an unknown method inside a batch", async () => {
    const res = await request(app)
      .post("/")
      .send([{ jsonrpc: "2.0", id: 10, method: "ghost" }]);
    expect(res.status).toBe(200);
    expect(Array.isArray(res.body)).toBe(true);
    expect(res.body[0].error.code).toBe(-32601);
    expect(res.body[0].id).toBe(10);
  });

  it("returns inline -32600 for a malformed item inside a batch", async () => {
    const res = await request(app)
      .post("/")
      .send([{ id: 5 }]); // missing jsonrpc and method
    expect(Array.isArray(res.body)).toBe(true);
    expect(res.body[0].error.code).toBe(-32600);
  });

  it("proxies valid batch items to upstream and returns array", async () => {
    // Both items are structurally valid — upstream will refuse the connection,
    // so each item comes back as a -32603 upstream error.
    const res = await request(app)
      .post("/")
      .send([
        { jsonrpc: "2.0", id: 1, method: "getBalance", params: ["pub1"] },
        { jsonrpc: "2.0", id: 2, method: "getSlot" },
      ]);
    expect(Array.isArray(res.body)).toBe(true);
    expect(res.body.length).toBe(2);
    res.body.forEach((r) => {
      expect(r.error.code).toBe(-32603);
    });
  });
});

// ─── OPTIONS preflight ────────────────────────────────────────────────────────
describe("OPTIONS preflight", () => {
  const app = buildApp();

  it("returns 204 for OPTIONS /", async () => {
    const res = await request(app).options("/");
    expect(res.status).toBe(204);
  });
});

// ─── GET /endpoints ───────────────────────────────────────────────────────────
describe("GET /endpoints", () => {
  it("returns 200 with JSON content-type", async () => {
    const app = buildApp();
    const res = await request(app).get("/endpoints");
    expect(res.status).toBe(200);
    expect(res.headers["content-type"]).toMatch(/application\/json/);
  });

  it("returns count and endpoints array", async () => {
    const app = buildApp();
    const res = await request(app).get("/endpoints");
    expect(typeof res.body.count).toBe("number");
    expect(Array.isArray(res.body.endpoints)).toBe(true);
    expect(res.body.count).toBe(res.body.endpoints.length);
  });

  it("reflects a single-URL pool", async () => {
    const app = buildApp("https://api.devnet.solana.com");
    const res = await request(app).get("/endpoints");
    expect(res.body.count).toBe(1);
    expect(res.body.endpoints).toEqual(["https://api.devnet.solana.com"]);
  });

  it("reflects a multi-URL pool", async () => {
    const urls = [
      "https://api.mainnet-beta.solana.com",
      "https://rpc.ankr.com/solana",
      "https://solana.drpc.org",
    ];
    const app = buildApp(urls);
    const res = await request(app).get("/endpoints");
    expect(res.body.count).toBe(3);
    expect(res.body.endpoints).toEqual(urls);
  });

  it("default pool includes the public mainnet list (≥10 endpoints)", async () => {
    const app = buildApp();
    const res = await request(app).get("/endpoints");
    expect(res.body.count).toBeGreaterThanOrEqual(10);
  });

  it("sets CORS header", async () => {
    const app = buildApp();
    const res = await request(app).get("/endpoints");
    expect(res.headers["access-control-allow-origin"]).toBe("*");
  });
});

// ─── Multi-URL pool failover ───────────────────────────────────────────────────
describe("POST / (multi-URL pool failover)", () => {
  it("returns 502 when all pool endpoints are unreachable", async () => {
    // Both ports are intentionally unreachable.
    const app = buildApp([
      "http://127.0.0.1:19997",
      "http://127.0.0.1:19998",
    ]);
    const res = await request(app)
      .post("/")
      .send({ jsonrpc: "2.0", id: 1, method: "getBalance", params: ["pub"] });
    expect(res.status).toBe(502);
    expect(res.body.error.code).toBe(-32603);
  });

  it("method validation still works with a multi-URL pool", async () => {
    const app = buildApp([
      "http://127.0.0.1:19997",
      "http://127.0.0.1:19998",
    ]);
    const res = await request(app)
      .post("/")
      .send({ jsonrpc: "2.0", id: 9, method: "notAMethod" });
    expect(res.status).toBe(400);
    expect(res.body.error.code).toBe(-32601);
  });
});
