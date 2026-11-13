type DirectoryListingGate = {
  requested: boolean
  release: () => void
  wait: Promise<void>
}

const gates = new Map<string, DirectoryListingGate>()

/** Hold one fixture listing so E2E tests can control response ordering. */
export function holdNextDirectoryListing(path: string) {
  let release!: () => void
  const wait = new Promise<void>((resolve) => {
    release = resolve
  })
  gates.set(path, { requested: false, release, wait })
}

export function directoryListingWasRequested(path: string) {
  return gates.get(path)?.requested ?? false
}

export function releaseDirectoryListing(path: string) {
  const gate = gates.get(path)
  if (!gate) throw new Error(`No delayed directory listing for ${path}`)
  gate.release()
}

export async function waitForDirectoryListingRelease(path: string) {
  const gate = gates.get(path)
  if (!gate || gate.requested) return
  gate.requested = true
  await gate.wait
  gates.delete(path)
}
