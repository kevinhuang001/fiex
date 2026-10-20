export interface DialogState {
  title: string
  label?: string
  value?: string
  submit: string
  action: (value: string) => Promise<unknown>
  danger?: boolean
}
