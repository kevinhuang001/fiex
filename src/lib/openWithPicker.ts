export interface OpenWithPickerItem {
  id: string
  name: string
  iconSource: string | null
}

export interface OpenWithPickerResponse {
  sessionId: string
  items: OpenWithPickerItem[]
}
