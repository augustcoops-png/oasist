"use strict";

const request = require("supertest");
const { buildApp } = require("../server");

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

describe("POST /", () => {
  // Use a custom rpcUrl that will always fail to connect so we can test the
  // upstream-error branch without making real network calls.
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

describe("OPTIONS preflight", () => {
  const app = buildApp();

  it("returns 204 for OPTIONS /", async () => {
    const res = await request(app).options("/");
    expect(res.status).toBe(204);
  });
});
