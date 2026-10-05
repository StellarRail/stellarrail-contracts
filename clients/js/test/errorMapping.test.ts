import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { ESCROW_ERRORS, mapContractError } from "../src/errorMapping.js";

describe("error mapping", () => {
  it("maps all 11 codes to names", () => {
    const names = [
      "AlreadyExists",
      "NotFound",
      "Unauthorized",
      "InvalidAmount",
      "InvalidDeadline",
      "InvalidState",
      "Expired",
      "NotExpired",
      "Overflow",
      "Paused",
      "AlreadyInitialized",
    ];
    names.forEach((name, i) => {
      const code = i + 1;
      assert.equal(ESCROW_ERRORS[code as keyof typeof ESCROW_ERRORS].name, name);
    });
  });

  it("parses simulation error fragments with HTTP codes", () => {
    assert.deepEqual(mapContractError("error: EscrowError(3)"), {
      code: 3,
      name: "Unauthorized",
      http: 403,
    });
    assert.deepEqual(mapContractError("Error(Contract, #7)"), {
      code: 7,
      name: "Expired",
      http: 410,
    });
    assert.deepEqual(mapContractError("Error(Contract, #10)"), {
      code: 10,
      name: "Paused",
      http: 503,
    });
  });

  it("handles unknown codes and non-errors", () => {
    assert.deepEqual(mapContractError("Error(Contract, #99)"), {
      code: 99,
      name: "Unknown",
      http: 500,
    });
    assert.equal(mapContractError("all good"), null);
  });
});
