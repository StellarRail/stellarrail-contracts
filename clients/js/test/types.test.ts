import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  bytesN32ToUuid,
  stroopsToXlm,
  uuidToBytesN32,
  xlmToStroops,
} from "../src/types.js";

describe("stroops conversions", () => {
  it("xlmToStroops handles whole and fractional XLM", () => {
    assert.equal(xlmToStroops("1"), "10000000");
    assert.equal(xlmToStroops("1.5"), "15000000");
    assert.equal(xlmToStroops("0.0000001"), "1");
    assert.equal(xlmToStroops("50000000"), "500000000000000");
  });

  it("stroopsToXlm trims and round-trips", () => {
    assert.equal(stroopsToXlm("10000000"), "1");
    assert.equal(stroopsToXlm("15000000"), "1.5");
    assert.equal(stroopsToXlm("1"), "0.0000001");
    assert.equal(stroopsToXlm(xlmToStroops("123.456789")), "123.456789");
  });
});

describe("uuid <-> BytesN<32>", () => {
  it("uuidToBytesN32 pads a uuid to 64 hex", () => {
    assert.equal(
      uuidToBytesN32("9f2c1111-2222-3333-4444-555555555555"),
      "9f2c1111222233334444555555555555" + "0".repeat(32),
    );
  });

  it("bytesN32ToUuid recovers the uuid and rejects garbage", () => {
    const u = "9f2c1111-2222-3333-4444-555555555555";
    assert.equal(bytesN32ToUuid(uuidToBytesN32(u)), u);
    assert.throws(() => bytesN32ToUuid("zz"), /invalid bytesn32/);
    assert.throws(() => uuidToBytesN32("nope"), /invalid uuid/);
  });
});
