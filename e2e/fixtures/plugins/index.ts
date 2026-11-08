import { fixturePlugins as fixturePluginCatalog } from './fixtures'
import { officialMode, officialPlugins, query } from './official'
import { marketplace } from './marketplace'
import type { FixturePlugin } from './types'

export { marketplace, officialMode, officialPlugins, query }

export const fixturePlugins: FixturePlugin[] = officialMode
  ? structuredClone(officialPlugins)
  : query.has('pluginFixture')
    ? structuredClone(fixturePluginCatalog)
    : []
