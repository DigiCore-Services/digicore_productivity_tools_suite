/**
 * TauRPC type-safe IPC proxy singleton.
 * Use this instead of invoke() for all backend calls.
 */
import { createTauRPCProxy } from "../bindings";

let _proxy: ReturnType<typeof createTauRPCProxy> | null = null;

export function getTaurpc() {
  if (!_proxy) {
    _proxy = createTauRPCProxy();
  }
  return _proxy;
}
