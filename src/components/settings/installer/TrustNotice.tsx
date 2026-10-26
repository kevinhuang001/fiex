import type { InstallPreview } from './types'

/** Package signature note. Trust is the user's judgement of the source, so the
 *  import screen shows developer information and a general warning instead of
 *  claiming a publisher is trusted or untrusted. */
export function TrustNotice({ preview }: { preview: InstallPreview | null }) {
  if (!preview) return null
  return (
    <div className="plugin-signature-note">
      <strong>{preview.signed ? '插件包已签名' : '插件包未签名'}</strong>
      <span>请只安装你信任来源的插件，并确认下面的权限。</span>
    </div>
  )
}
