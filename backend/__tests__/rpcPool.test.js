"use strict";

const { RpcPool, PUBLIC_ENDPOINTS, parseUrls } = require("../rpcPool");

// ─── parseUrls ────────────────────────────────────────────────────────────────
describe("parseUrls", () => {
  it("returns empty array for undefined input", () => {
    expect(parseUrls(undefined)).toEqual([]);
  });

  it("returns empty array for empty string", () => {
    expect(parseUrls("")).toEqual([]);
  });

  it("parses a single URL", () => {
    expect(parseUrls("https://example.com")).toEqual(["https://example.com"]);
  });

  it("parses a comma-separated list", () => {
    expect(parseUrls("https://a.com,https://b.com,https://c.com")).toEqual([
      "https://a.com",
      "https://b.com",
      "https://c.com",
    ]);
  });

  it("trims whitespace around each URL", () => {
    expect(parseUrls("  https://a.com ,  https://b.com  ")).toEqual([
      "https://a.com",
      "https://b.com",
    ]);
  });

  it("filters out empty entries (e.g. trailing comma)", () => {
    expect(parseUrls("https://a.com,,https://b.com,")).toEqual([
      "https://a.com",
      "https://b.com",
    ]);
  });
});

// ─── PUBLIC_ENDPOINTS ─────────────────────────────────────────────────────────
describe("PUBLIC_ENDPOINTS", () => {
  it("has mainnet, devnet, and testnet keys", () => {
    expect(PUBLIC_ENDPOINTS).toHaveProperty("mainnet");
    expect(PUBLIC_ENDPOINTS).toHaveProperty("devnet");
    expect(PUBLIC_ENDPOINTS).toHaveProperty("testnet");
  });

  it("mainnet list has at least 10 endpoints", () => {
    expect(Array.isArray(PUBLIC_ENDPOINTS.mainnet)).toBe(true);
    expect(PUBLIC_ENDPOINTS.mainnet.length).toBeGreaterThanOrEqual(10);
  });

  it("mainnet list includes the official Solana Labs endpoint", () => {
    expect(PUBLIC_ENDPOINTS.mainnet).toContain(
      "https://api.mainnet-beta.solana.com"
    );
  });

  it("devnet list includes the official Solana Labs devnet endpoint", () => {
    expect(PUBLIC_ENDPOINTS.devnet).toContain("https://api.devnet.solana.com");
  });

  it("testnet list includes the official Solana Labs testnet endpoint", () => {
    expect(PUBLIC_ENDPOINTS.testnet).toContain(
      "https://api.testnet.solana.com"
    );
  });

  it("every mainnet entry is a valid https URL", () => {
    PUBLIC_ENDPOINTS.mainnet.forEach((url) => {
      expect(() => new URL(url)).not.toThrow();
      expect(url.startsWith("https://") || url.startsWith("http://")).toBe(
        true
      );
    });
  });
});

// ─── RpcPool constructor ──────────────────────────────────────────────────────
describe("RpcPool constructor", () => {
  it("constructs with a single URL", () => {
    const pool = new RpcPool(["https://example.com"]);
    expect(pool.size()).toBe(1);
  });

  it("constructs with multiple URLs", () => {
    const pool = new RpcPool([
      "https://a.com",
      "https://b.com",
      "https://c.com",
    ]);
    expect(pool.size()).toBe(3);
  });

  it("throws when constructed with an empty array", () => {
    expect(() => new RpcPool([])).toThrow();
  });

  it("throws when constructed with a non-array", () => {
    expect(() => new RpcPool("https://example.com")).toThrow();
  });
});

// ─── RpcPool.endpoints() ─────────────────────────────────────────────────────
describe("RpcPool.endpoints()", () => {
  it("returns a copy of all URLs in insertion order", () => {
    const urls = ["https://a.com", "https://b.com"];
    const pool = new RpcPool(urls);
    expect(pool.endpoints()).toEqual(urls);
  });

  it("returns a copy — mutating it does not affect the pool", () => {
    const pool = new RpcPool(["https://a.com", "https://b.com"]);
    const copy = pool.endpoints();
    copy.push("https://evil.com");
    expect(pool.size()).toBe(2);
  });
});

// ─── RpcPool.pick() — round-robin ────────────────────────────────────────────
describe("RpcPool.pick()", () => {
  it("returns the first URL on the first call", () => {
    const pool = new RpcPool(["https://a.com", "https://b.com"]);
    expect(pool.pick()).toBe("https://a.com");
  });

  it("advances to the next URL on each call", () => {
    const pool = new RpcPool([
      "https://a.com",
      "https://b.com",
      "https://c.com",
    ]);
    expect(pool.pick()).toBe("https://a.com");
    expect(pool.pick()).toBe("https://b.com");
    expect(pool.pick()).toBe("https://c.com");
  });

  it("wraps back to the first URL after exhausting all", () => {
    const pool = new RpcPool(["https://a.com", "https://b.com"]);
    pool.pick(); // a
    pool.pick(); // b
    expect(pool.pick()).toBe("https://a.com"); // wraps
  });

  it("distributes picks evenly across all endpoints (round-robin)", () => {
    const urls = ["https://a.com", "https://b.com", "https://c.com"];
    const pool = new RpcPool(urls);
    const counts = { "https://a.com": 0, "https://b.com": 0, "https://c.com": 0 };
    for (let i = 0; i < 30; i++) {
      counts[pool.pick()]++;
    }
    Object.values(counts).forEach((c) => expect(c).toBe(10));
  });
});

// ─── RpcPool.ordered() — fallback ordering ───────────────────────────────────
describe("RpcPool.ordered()", () => {
  it("returns all URLs starting from the current pointer", () => {
    const pool = new RpcPool(["https://a.com", "https://b.com", "https://c.com"]);
    // Pointer starts at 0 → should start with a.com
    expect(pool.ordered()).toEqual([
      "https://a.com",
      "https://b.com",
      "https://c.com",
    ]);
  });

  it("wraps around so every URL appears exactly once", () => {
    const urls = ["https://a.com", "https://b.com", "https://c.com"];
    const pool = new RpcPool(urls);
    pool.pick(); // advance to 1
    pool.pick(); // advance to 2
    const ordered = pool.ordered(); // starts at 2 → [c, a, b]
    expect(ordered).toHaveLength(3);
    expect(new Set(ordered)).toEqual(new Set(urls));
    expect(ordered[0]).toBe("https://c.com");
  });

  it("advances the pointer by 1 after each call", () => {
    const pool = new RpcPool(["https://a.com", "https://b.com", "https://c.com"]);
    pool.ordered(); // consumes pointer 0 → pointer now 1
    expect(pool.pick()).toBe("https://b.com"); // pointer was 1, advances to 2
  });

  it("returns a list of length equal to pool size", () => {
    const urls = ["https://a.com", "https://b.com"];
    const pool = new RpcPool(urls);
    expect(pool.ordered()).toHaveLength(urls.length);
  });
});
