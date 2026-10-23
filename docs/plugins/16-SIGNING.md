# Plugin signing

`.fiexplugin` is a ZIP package. Signing is detached and does not change the
native executable.

## Package layout

```text
manifest.json
manifest.sig
fiex-plugin-*
ui/dist/index.html
```

`manifest.json` carries:

```json
{
  "publisher": {
    "id": "com.example.publisher",
    "name": "Example Publisher",
    "keyId": "sha256...",
    "publicKey": "base64-ed25519-public-key"
  },
  "files": {
    "fiex-plugin-example": "sha256:...",
    "ui/dist/index.html": "sha256:..."
  }
}
```

The signature is Ed25519 over:

```text
"fiex-plugin-manifest-v1\0" + exact manifest.json bytes
```

`manifest.files` covers every package file except `manifest.json` and
`manifest.sig`. Core verifies the signature, then every file digest. It also
re-checks the package before starting an installed plugin.

## Trust

There is no special official key. Every publisher, including Fiex, has an
Ed25519 key and is trusted on first use:

1. Core verifies the signature cryptographically.
2. The install review shows the publisher ID and key fingerprint.
3. The user explicitly confirms before Core stores the publisher key in
   `plugin-trust.json`.
4. Later versions must match the same publisher ID and key. A changed key is
   rejected until the user removes the old trust entry and trusts the new key.

Unsigned packages are still installable with an explicit unsigned warning, so
old packages and local experiments remain possible. They do not gain publisher
trust.

## Signing a package

```bash
python -m pip install cryptography
python scripts/sign-plugin-package.py my-plugin.fiexplugin \
  --key publisher-private-key.pem \
  --publisher-id com.example.publisher \
  --publisher-name "Example Publisher"
```

Generate a key with:

```bash
openssl genpkey -algorithm ED25519 -out publisher-private-key.pem
```

Keep the private key out of the repository and release artifacts. Only the
public key embedded in the signed manifest is distributed.
