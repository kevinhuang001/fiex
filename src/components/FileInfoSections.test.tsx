import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { FileInfoSections } from './FileInfoSections'
import type { InfoSection } from '@/lib/ipcValidation/infoTypes'

describe('FileInfoSections', () => {
  it('renders one flat visible section per provider and hides subsection identities', () => {
    const sections: InfoSection[] = [
      {
        id: 'core.metadata',
        title: 'Metadata',
        providerId: 'core.metadata',
        provider: 'Fiex',
        source: 'core',
        fields: [
          {
            id: 'name',
            subsectionId: 'basic',
            label: 'Name',
            compute: 'onOpen',
            status: 'ready',
            value: 'song.mp3',
          },
        ],
      },
      {
        id: 'core.media',
        title: 'Media',
        providerId: 'core.media',
        provider: 'Fiex',
        source: 'core',
        fields: [
          {
            id: 'duration',
            subsectionId: 'audio',
            label: 'Duration',
            compute: 'onDemand',
            status: 'idle',
          },
          {
            id: 'duration',
            subsectionId: 'container',
            label: 'Container duration',
            compute: 'onDemand',
            status: 'idle',
          },
        ],
      },
    ]
    const markup = renderToStaticMarkup(
      <FileInfoSections
        sections={sections}
        copied=""
        computeDisabled={false}
        onCopy={() => {}}
        onCompute={() => {}}
      />,
    )

    expect(markup.match(/class="file-info-provider-section"/g)).toHaveLength(2)
    expect(markup.match(/class="file-info-provider-title"/g)).toHaveLength(2)
    expect(markup).toContain('<h4 class="file-info-provider-title">Metadata</h4>')
    expect(markup).toContain('<h4 class="file-info-provider-title">Media</h4>')
    expect(markup.match(/class="file-info-row"/g)).toHaveLength(3)
    expect(markup).not.toContain('basic')
    expect(markup).not.toContain('audio')
    expect(markup).not.toContain('container')
  })
})
