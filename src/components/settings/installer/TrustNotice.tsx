import type { InstallPreview } from './types'

/** Signature/publisher warnings and the verified-signature notice. */
export function TrustNotice({ preview }: { preview: InstallPreview | null }) {
  return (
    <>
      {preview && !preview.trusted && (
        <div className="plugin-signature-warning">
          {preview.signed ? (
            <>
              <strong>未信任的发布者</strong>
              <span>
                {preview.publisherName || preview.publisher || '未知发布者'} · 密钥指纹{' '}
                <code>{preview.fingerprint || preview.keyId}</code>
              </span>
            </>
          ) : (
            <>
              <strong>此插件未签名</strong>
              <span>只在你确认来源可信时继续安装。</span>
            </>
          )}
        </div>
      )}
      {preview?.signed && preview.trusted && (
        <div className="plugin-signature-ok">
          签名已验证 · {preview.publisherName || preview.publisher}
        </div>
      )}
    </>
  )
}
