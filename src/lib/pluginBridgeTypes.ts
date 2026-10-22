/** Host-side type contract: importing these browser SDK types ensures the
 * iframe Bridge schema is checked by Fiex's existing TypeScript build. */
export type {
  BridgeRequest,
  BridgeResponse,
  BridgeError,
  ExplorerContext,
  ResourceRef,
} from '../../sdk/src/index'

export type { HostSession } from '../../sdk/src/host'
