/** Browser-side safeguard for command errors and diagnostic exports. */
export function redactDiagnostic(input: string, limit = 4000): string {
  if (input.length > 256 * 1024) return '[diagnostic omitted: size limit]'
  return input
    .replace(
      /((?:authorization|proxy-authorization|cookie|set-cookie|x-[\w-]*(?:key|token|secret))\s*[:=]\s*)[^\r\n]*(?:\r?\n[ \t]+[^\r\n]*)*/gim,
      '$1[REDACTED]',
    )
    .replace(
      /((?:[\w-]*(?:token|secret|password|passwd|credential)[\w-]*|api[_-]?key|key|code)\s*["']?\s*[:=]\s*)(?:"(?:[^"\\]|\\.)*"|'(?:[^'\\]|\\.)*'|[^\s&,;}\]"']+)/gi,
      '$1[REDACTED]',
    )
    .replace(/(\b(?:bearer|basic)\s+)[a-z0-9+/=_\-.]+/gi, '$1[REDACTED]')
    .replace(/[\u0000-\u0008\u000b-\u001f\u007f]/g, '')
    .slice(0, limit)
    .trim()
}
