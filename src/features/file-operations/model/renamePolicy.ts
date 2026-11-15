export interface RenameCapability {
  allowedOps?: readonly string[]
}

/** An entry can be renamed only when its provider explicitly grants the operation. */
export function canRenameEntry(entry: RenameCapability | undefined): boolean {
  return entry?.allowedOps?.includes('rename') ?? false
}
