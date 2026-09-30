/** Canonical escrow shapes (mirror `src/types.rs` + `src/storage.rs`). */

export type EscrowStatus =
  | "Created"
  | "Locked"
  | "Released"
  | "Refunded"
  | "Expired"
  | "Failed";

export interface EscrowRequest {
  /** 64-hex request id (BytesN<32>). */
  requestId: string;
  /** Strkey (G...) funder. */
  depositor: string;
  /** Strkey payout target, if set. */
  destination?: string;
  /** Stroops as decimal string (i128 — never a float). */
  amount: string;
  /** Unix seconds. */
  deadline: number;
  status: EscrowStatus;
  createdAt: number;
  updatedAt: number;
}

export interface EscrowStats {
  lockedCount: number;
  /** Stroops, decimal string. */
  lockedTotal: string;
  releasedCount: number;
  /** Stroops, decimal string. */
  lifetimeVolume: string;
}

export const STROOPS_PER_XLM = 10_000_000n;

/** XLM (decimal string) → stroops (decimal string). No floats involved. */
export function xlmToStroops(xlm: string): string {
  const [whole = "0", frac = ""] = xlm.split(".");
  const fracPadded = (frac + "0000000").slice(0, 7);
  return (BigInt(whole) * STROOPS_PER_XLM + BigInt(fracPadded)).toString();
}

/** Stroops (decimal string) → XLM (decimal string, trimmed). */
export function stroopsToXlm(stroops: string): string {
  const v = BigInt(stroops);
  const whole = v / STROOPS_PER_XLM;
  const frac = (v % STROOPS_PER_XLM).toString().padStart(7, "0").replace(/0+$/, "");
  return frac === "" ? whole.toString() : `${whole}.${frac}`;
}

/** UUID (with dashes) → 64-hex `BytesN<32>` for `request_id`. */
export function uuidToBytesN32(uuid: string): string {
  const hex = uuid.replace(/-/g, "").toLowerCase();
  if (!/^[0-9a-f]{32}$/.test(hex)) throw new Error(`invalid uuid: ${uuid}`);
  return hex + "0".repeat(32);
}

/** 64-hex `BytesN<32>` → uuid (first 16 bytes). */
export function bytesN32ToUuid(bytesN: string): string {
  const h = bytesN.toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(h)) throw new Error(`invalid bytesn32: ${bytesN}`);
  const u = h.slice(0, 32);
  return `${u.slice(0, 8)}-${u.slice(8, 12)}-${u.slice(12, 16)}-${u.slice(16, 20)}-${u.slice(20)}`;
}
