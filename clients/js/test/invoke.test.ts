import { before, describe, it } from "node:test";
import assert from "node:assert/strict";
import { chmodSync, mkdtempSync, readFileSync, symlinkSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { deposit, getStats, type NetConfig } from "../src/invoke.js";

const here = dirname(fileURLToPath(import.meta.url));
// At runtime `here` is dist-test/test; the fake CLI lives in test/bin.
const fakeStellar = join(here, "..", "..", "test", "bin", "fake-stellar.sh");
let logPath = "";
const net: NetConfig = {
  rpcUrl: "https://example.invalid",
  networkPassphrase: "Test SDF Network ; September 2015",
  contractId: "CAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
  source: "test-source",
};

before(() => {
  // Shadow the real CLI with the fake binary for the whole file.
  const bin = mkdtempSync(join(tmpdir(), "fake-stellar-"));
  chmodSync(fakeStellar, 0o755);
  symlinkSync(fakeStellar, join(bin, "stellar"));
  process.env.PATH = `${bin}:${process.env.PATH ?? ""}`;
  logPath = join(bin, "argv.log");
  process.env.FAKE_STELLAR_LOG = logPath;
});

describe("invoke arg shapes (mocked CLI)", () => {
  it("deposit builds the full arg list with quoted destination", async () => {
    const res = await deposit(net, {
      amount: "10000000",
      requestId: "ab".repeat(32),
      depositor: "GDEPOSITOR",
      destination: "GDEST",
      deadline: 123,
    });
    assert.equal(res.status, "Locked");
    const logged: string = readFileSync(logPath, "utf8");
    assert.match(logged, /deposit/);
    assert.match(logged, /--amount 10000000/);
    assert.match(logged, /--request-id (ab){32}/);
    assert.match(logged, /--depositor GDEPOSITOR/);
    assert.match(logged, /--destination "GDEST"/);
    assert.match(logged, /--deadline 123/);
  });

  it("getStats hits the view path", async () => {
    const stats = await getStats(net);
    assert.equal(stats.lockedCount, 0);
    assert.equal(stats.lifetimeVolume, "0");
    const logged: string = readFileSync(logPath, "utf8");
    assert.match(logged, /get_stats/);
  });
});
