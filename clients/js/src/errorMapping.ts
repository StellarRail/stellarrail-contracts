/** Contract error codes (mirror `src/errors.rs`) → API mapping. */

export const ESCROW_ERRORS = {
  1: { name: "AlreadyExists", http: 409 },
  2: { name: "NotFound", http: 404 },
  3: { name: "Unauthorized", http: 403 },
  4: { name: "InvalidAmount", http: 400 },
  5: { name: "InvalidDeadline", http: 400 },
  6: { name: "InvalidState", http: 409 },
  7: { name: "Expired", http: 410 },
  8: { name: "NotExpired", http: 409 },
  9: { name: "Overflow", http: 500 },
  10: { name: "Paused", http: 503 },
  11: { name: "AlreadyInitialized", http: 409 },
} as const;

export type EscrowErrorCode = keyof typeof ESCROW_ERRORS;

export interface MappedError {
  code: number;
  name: string;
  http: number;
}

/** Parse `EscrowError(N)` / `Error(Contract, #N)` fragments into a mapping. */
export function mapContractError(message: string): MappedError | null {
  const m = /(?:EscrowError\(|Contract, #)(\d+)/.exec(message);
  if (m === null || m[1] === undefined) return null;
  const code = Number(m[1]);
  const entry = (ESCROW_ERRORS as Record<number, { name: string; http: number }>)[code];
  if (entry === undefined) return { code, name: "Unknown", http: 500 };
  return { code, name: entry.name, http: entry.http };
}
