import { t } from '@/i18n'
import { HardDrive, Usb } from 'lucide-react'
import { Fragment } from 'react'
import { size } from '@/lib/files'
import type { SetSidebarMenu, SidebarProps } from './types'

const DISK_COLORS = ['#5b9cf8', '#55b98a', '#e0a44a', '#b47ad6', '#e06c75', '#56b6c9']

/** Disks and USB volumes with capacity meters and eject/mount buttons. */
export function Devices({
  x,
  onMount,
  setMenu,
}: Pick<SidebarProps, 'x' | 'onMount'> & {
  setMenu: SetSidebarMenu
}) {
  return (
    <Fragment>
      <div className="sidebar-section-title devices-title">
        <span className="sidebar-label">{t('设备')}</span>
      </div>
      {x.locations
        .filter((location) => location.kind === 'disk')
        .map((location, index) => {
          const color = DISK_COLORS[index % DISK_COLORS.length]
          const used =
            location.total > 0
              ? Math.min(100, Math.max(0, (1 - location.available / location.total) * 100))
              : 0
          // Removable volumes are ejectable even when the OS mounts them at a
          // Windows drive letter; fixed system volumes stay protected.
          const ejectable =
            Boolean(location.usbfs) ||
            (location.mounted &&
              location.path !== '/' &&
              (location.removable || !/^[A-Za-z]:[\\/]?$/.test(location.path)))
          return (
            <div
              className="device-row"
              key={location.path || location.device}
              onContextMenu={(event) => {
                event.preventDefault()
                if (location.mounted && !ejectable) return
                setMenu({
                  kind: 'device',
                  x: event.clientX,
                  y: event.clientY,
                  name: location.name || location.path || location.device,
                  device: location.device,
                  path: location.path,
                  mounted: location.mounted,
                  usbfs: Boolean(location.usbfs),
                  canEject: ejectable,
                })
              }}
            >
              <button
                className={`nav-item disk-item ${location.mounted || location.usbfs ? '' : 'disk-unmounted'} ${x.path === location.path ? 'active' : ''}`}
                onClick={() =>
                  location.path ? x.navigate(location.path) : onMount(location.device)
                }
                title={location.path ? location.path : t('未挂载，点击授权挂载')}
              >
                {location.removable ? <Usb color={color} /> : <HardDrive color={color} />}
                <span>
                  <span>{location.name || location.path}</span>
                  <small
                    className="disk-capacity"
                    title={t('可用 {0} / 总计 {1}', size(location.available), size(location.total))}
                  >
                    {!location.mounted
                      ? size(location.total)
                      : location.total > 0
                        ? `${size(location.available)} / ${size(location.total)}`
                        : size(location.available)}
                    {location.filesystem && (
                      <span className="disk-fs">
                        {location.filesystem.toUpperCase()}
                        {location.usbfs
                          ? ['APFS', 'HFS', 'HFS+', 'HFSX', 'BTRFS'].includes(
                              location.filesystem.toUpperCase(),
                            )
                            ? t(' · 只读')
                            : t(' · 内置')
                          : !location.mounted
                            ? t(' · 挂载')
                            : ''}
                      </span>
                    )}
                  </small>
                  {location.total > 0 && (
                    <span
                      className="disk-meter"
                      role="progressbar"
                      aria-label={t(
                        '{0} 已用 {1}%',
                        location.name || location.path,
                        Math.round(used),
                      )}
                      aria-valuenow={Math.round(used)}
                      aria-valuemin={0}
                      aria-valuemax={100}
                    >
                      <i style={{ width: `${used}%`, background: color }} />
                    </span>
                  )}
                </span>
              </button>
            </div>
          )
        })}
    </Fragment>
  )
}
