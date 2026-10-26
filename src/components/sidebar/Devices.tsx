import { t } from '@/i18n'
import { HardDrive, Unplug, Usb } from 'lucide-react'
import { Fragment } from 'react'
import { size } from '@/lib/files'
import type { SidebarProps } from './types'

const DISK_COLORS = ['#5b9cf8', '#55b98a', '#e0a44a', '#b47ad6', '#e06c75', '#56b6c9']

/** Disks and USB volumes with capacity meters and eject/mount buttons. */
export function Devices({
  x,
  onEject,
  onEjectDevice,
  onMount,
}: Pick<SidebarProps, 'x' | 'onEject' | 'onEjectDevice' | 'onMount'>) {
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
          return (
            <div className="device-row" key={location.path || location.device}>
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
              {location.usbfs ? (
                <button
                  className="device-eject icon-button"
                  title={t('弹出 U 盘')}
                  onClick={() => onEjectDevice(location.device)}
                >
                  <Unplug size={13} />
                </button>
              ) : (
                location.mounted &&
                location.path !== '/' &&
                !/^[A-Za-z]:[\\/]?$/.test(location.path) && (
                  <button
                    className="device-eject icon-button"
                    title={t('弹出设备')}
                    onClick={() => onEject(location.path)}
                  >
                    <Unplug size={13} />
                  </button>
                )
              )}
            </div>
          )
        })}
    </Fragment>
  )
}
