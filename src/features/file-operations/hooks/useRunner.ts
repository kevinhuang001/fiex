import { useRef } from 'react'
import type { Dispatch, SetStateAction } from 'react'
import { createRunner } from '../services/createRunner'
import type { FileOperationRuntimeServices, FileOperationServices } from '../types'
import type { ExplorerOperationState } from '../types'

/** React lifetime adapter for the serialized, dependency-injected runner. */
export function useRunner({
  x,
  setBusy,
  runtime,
  services,
}: {
  x: Pick<ExplorerOperationState, 'path' | 'selected' | 'setError'>
  setBusy: Dispatch<SetStateAction<string>>
  runtime: FileOperationRuntimeServices
  services: Pick<FileOperationServices, 'reportPermissionError'>
}) {
  const running = useRef(false)
  const run = createRunner({
    x,
    running,
    setBusy,
    runtime,
    reportPermissionError: services.reportPermissionError,
  })
  return { run }
}
