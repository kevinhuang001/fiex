import { fixturePlugins } from './fixtures'
import { officialMode, officialPlugins, query } from './official'
import { marketplace } from './marketplace'
import type { DemoPlugin } from './types'

export { fixturePlugins, marketplace, officialMode, officialPlugins, query }

export const demoPlugins: DemoPlugin[] = officialMode
  ? structuredClone(officialPlugins)
  : query.has('pluginFixture')
    ? structuredClone(fixturePlugins)
    : []
