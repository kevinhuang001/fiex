export interface DialogState {
  title: string
  label?: string
  value?: string
  description?: string
  submit: string
  action: (value: string) => Promise<unknown>
  danger?: boolean
}
