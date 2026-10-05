/**
 * Minimal `stellar contract invoke` wrappers for the chain service.
 *
 * These shell out to the Stellar CLI (which owns signing + simulation),
 * keeping this package dependency-free. For in-process signing, port the
 * arg shapes below to `stellar-sdk`'s `Contract.call`.
 */
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import type { EscrowRequest, EscrowStats, EscrowStatus } from "./types.js";

const exec = promisify(execFile);

export interface NetConfig {
  rpcUrl: string;
  networkPassphrase: string;
  contractId: string;
  /** Secret key, seed, or CLI identity used for `--source`. */
  source: string;
}

function run<T>(args: string[]): Promise<T> {
  return exec("stellar", args, { maxBuffer: 16 * 1024 * 1024 }).then(({ stdout }) => {
    return JSON.parse(stdout) as T;
  });
}

function base(net: NetConfig): string[] {
  return [
    "contract",
    "invoke",
    "--id",
    net.contractId,
    "--rpc-url",
    net.rpcUrl,
    "--network-passphrase",
    net.networkPassphrase,
    "--source",
    net.source,
    "--",
  ];
}

export function deposit(
  net: NetConfig,
  p: { amount: string; requestId: string; depositor: string; destination: string; deadline: number },
): Promise<EscrowRequest> {
  // The CLI parses Option<Address> as JSON: quote a bare strkey.
  const destination = /^".*"$/.test(p.destination) ? p.destination : `"${p.destination}"`;
  return run<EscrowRequest>([
    ...base(net),
    "deposit",
    "--amount",
    p.amount,
    "--request-id",
    p.requestId,
    "--depositor",
    p.depositor,
    "--destination",
    destination,
    "--deadline",
    String(p.deadline),
  ]);
}

export function release(net: NetConfig, p: { caller: string; requestId: string }): Promise<EscrowRequest> {
  return run<EscrowRequest>([...base(net), "release", "--caller", p.caller, "--request-id", p.requestId]);
}

export function refund(net: NetConfig, p: { caller: string; requestId: string }): Promise<EscrowRequest> {
  return run<EscrowRequest>([...base(net), "refund", "--caller", p.caller, "--request-id", p.requestId]);
}

export function expire(
  net: NetConfig,
  p: { requestId: string; source: string },
): Promise<EscrowRequest> {
  return run<EscrowRequest>([
    "contract",
    "invoke",
    "--id",
    net.contractId,
    "--rpc-url",
    net.rpcUrl,
    "--network-passphrase",
    net.networkPassphrase,
    "--source",
    p.source,
    "--",
    "expire",
    "--request-id",
    p.requestId,
  ]);
}

export function getRequest(net: Omit<NetConfig, "source">, p: { requestId: string }): Promise<EscrowRequest> {
  return run<EscrowRequest>([
    "contract",
    "invoke",
    "--id",
    net.contractId,
    "--rpc-url",
    net.rpcUrl,
    "--network-passphrase",
    net.networkPassphrase,
    "--",
    "get_request",
    "--request-id",
    p.requestId,
  ]);
}

export function listRequests(
  net: Omit<NetConfig, "source">,
  p: { status?: EscrowStatus; offset: number; limit: number },
): Promise<EscrowRequest[]> {
  const args = [
    "contract",
    "invoke",
    "--id",
    net.contractId,
    "--rpc-url",
    net.rpcUrl,
    "--network-passphrase",
    net.networkPassphrase,
    "--",
    "list_requests",
    "--offset",
    String(p.offset),
    "--limit",
    String(p.limit),
  ];
  if (p.status !== undefined) args.push("--status-filter", p.status.toLowerCase());
  return run<EscrowRequest[]>(args);
}

export function getStats(net: Omit<NetConfig, "source">): Promise<EscrowStats> {
  return run<EscrowStats>([
    "contract",
    "invoke",
    "--id",
    net.contractId,
    "--rpc-url",
    net.rpcUrl,
    "--network-passphrase",
    net.networkPassphrase,
    "--",
    "get_stats",
  ]);
}
