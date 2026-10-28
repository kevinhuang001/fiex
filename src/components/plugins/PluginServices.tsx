import { PluginActivationHost } from './PluginActivationHost'
import { PluginPermissionHost } from './PluginPermissionHost'

export function PluginServices() {
  return (
    <>
      <PluginPermissionHost />
      <PluginActivationHost />
    </>
  )
}
