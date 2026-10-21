export const archiveFormats = [
  { id: 'zip', name: 'ZIP', ext: '.zip' },
  { id: '7z', name: '7Z', ext: '.7z' },
  { id: 'tar.gz', name: 'TAR.GZ', ext: '.tar.gz' },
  { id: 'tar', name: 'TAR', ext: '.tar' },
  { id: 'tar.bz2', name: 'TAR.BZ2', ext: '.tar.bz2' },
  { id: 'tar.xz', name: 'TAR.XZ', ext: '.tar.xz' },
] as const
export type ArchiveFormat = (typeof archiveFormats)[number]
