import type { InfoField, InfoOpenResult, InfoSection } from '@/lib/ipcValidation/infoTypes'

export interface InfoSessionServices {
  open(path: string, onUpdate: (value: unknown) => void): Promise<InfoOpenResult>
  compute(input: {
    sessionId: string
    providerId: string
    subsectionId: string
    fieldId: string
    taskId: string
  }): Promise<unknown>
  cancel(sessionId: string, taskId: string): Promise<void>
  close(sessionId: string): Promise<void>
  reportError(error: unknown, source: string): void
  createTaskId(): string
}

export interface InfoSessionState {
  sections: InfoSection[]
  loading: boolean
  computeDisabled: boolean
  compute(section: InfoSection, field: InfoField): Promise<void>
}
