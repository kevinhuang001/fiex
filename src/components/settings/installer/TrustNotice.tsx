import { t } from '@/i18n'
import type { InstallPreview } from './types'

/** Package signature details. Trust is the user's judgement of the source, so
 *  show who signed the package and the key fingerprint without claiming the
 *  publisher is trusted or untrusted. */
export function TrustNotice({ preview }: { preview: InstallPreview | null }) {
  if (!preview) return null
  const signer = preview.publisherName || preview.publisher || t('未知签名者')
  const fingerprint = preview.fingerprint || preview.keyId
  return (
    <div className="plugin-signature-note">
      <strong>{preview.signed ? t('插件包已签名') : t('插件包未签名')}</strong>
      {preview.signed && (
        <>
          <span>
            {t('签名者：')}
            {signer}
          </span>
          {fingerprint && (
            <span>
              {t('密钥指纹：')}
              <code>{fingerprint}</code>
            </span>
          )}
          <span>{t('校验方式：Core 内置 Ed25519')}</span>
        </>
      )}
      <span>{t('请只安装你信任来源的插件，并确认下面的权限。')}</span>
    </div>
  )
}
