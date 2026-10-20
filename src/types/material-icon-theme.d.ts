declare module 'material-icon-theme/dist/material-icons.json' {
  interface IconDefinition {
    iconPath: string
    fontCharacter?: string
    fontColor?: string
  }
  interface IconManifest {
    iconDefinitions: Record<string, IconDefinition>
    fileExtensions: Record<string, string>
    fileNames: Record<string, string>
    folderNames: Record<string, string>
    folderNamesExpanded: Record<string, string>
    file: string
    folder: string
    folderExpanded?: string
    rootFolder?: string
    light?: Partial<IconManifest>
    highContrast?: Partial<IconManifest>
  }
  const manifest: IconManifest
  export default manifest
}
