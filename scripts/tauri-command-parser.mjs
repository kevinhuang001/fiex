function rustCommandDefinitions(source) {
  const code = maskRustComments(source)
  const commands = []
  const attribute = /#\s*\[\s*tauri::command\b([^\]]*)\]/g
  for (const match of code.matchAll(attribute)) {
    const declaration = code.slice(match.index + match[0].length)
    const functionMatch = declaration.match(
      /(?:pub(?:\s*\([^)]*\))?\s+)?(?:async\s+)?fn\s+([A-Za-z_]\w*)\s*(?:<[^;{}]*?>\s*)?\(/,
    )
    if (!functionMatch) throw new Error('Could not read a function after #[tauri::command]')
    const open =
      match.index + match[0].length + functionMatch.index + functionMatch[0].lastIndexOf('(')
    const close = matchingDelimiter(code, open, '(', ')')
    const params = splitTopLevel(code.slice(open + 1, close), ',')
      .map(parseRustParameter)
      .filter((parameter) => parameter && !isTauriInjected(parameter.type))
    const renameRule = match[1].match(/rename_all\s*=\s*["'](camelCase|snake_case)["']/)?.[1]
    if (match[1].includes('rename_all') && !renameRule)
      throw new Error(`Unsupported Tauri rename_all rule on ${functionMatch[1]}`)
    commands.push({
      name: functionMatch[1],
      args: params.map((parameter) =>
        renameRule === 'snake_case' ? parameter.name : toCamelCase(parameter.name),
      ),
    })
  }
  return commands
}

function maskRustComments(source) {
  const characters = [...source]
  let quote = false
  let escaped = false
  let lineComment = false
  let blockDepth = 0
  for (let index = 0; index < characters.length; index += 1) {
    const char = characters[index]
    const next = characters[index + 1]
    if (lineComment) {
      if (char === '\n') lineComment = false
      else characters[index] = ' '
      continue
    }
    if (blockDepth) {
      if (char !== '\n') characters[index] = ' '
      if (char === '/' && next === '*') {
        characters[index + 1] = ' '
        blockDepth += 1
        index += 1
      } else if (char === '*' && next === '/') {
        characters[index + 1] = ' '
        blockDepth -= 1
        index += 1
      }
      continue
    }
    if (quote) {
      if (escaped) escaped = false
      else if (char === '\\') escaped = true
      else if (char === '"') quote = false
      continue
    }
    if (char === '"') quote = true
    else if (char === '/' && next === '/') {
      characters[index] = ' '
      characters[index + 1] = ' '
      lineComment = true
      index += 1
    } else if (char === '/' && next === '*') {
      characters[index] = ' '
      characters[index + 1] = ' '
      blockDepth = 1
      index += 1
    }
  }
  return characters.join('')
}

function matchingDelimiter(source, start, openChar, closeChar) {
  let depth = 0
  let quote = false
  let escaped = false
  let lineComment = false
  let blockDepth = 0
  for (let index = start; index < source.length; index += 1) {
    const char = source[index]
    const next = source[index + 1]
    if (lineComment) {
      if (char === '\n') lineComment = false
      continue
    }
    if (blockDepth) {
      if (char === '/' && next === '*') {
        blockDepth += 1
        index += 1
      } else if (char === '*' && next === '/') {
        blockDepth -= 1
        index += 1
      }
      continue
    }
    if (quote) {
      if (escaped) escaped = false
      else if (char === '\\') escaped = true
      else if (char === '"') quote = false
      continue
    }
    if (char === '/' && next === '/') {
      lineComment = true
      index += 1
      continue
    }
    if (char === '/' && next === '*') {
      blockDepth = 1
      index += 1
      continue
    }
    if (char === '"') {
      quote = true
      continue
    }
    if (char === openChar) depth += 1
    else if (char === closeChar && --depth === 0) return index
  }
  throw new Error(`Unclosed ${openChar}${closeChar} delimiter`)
}

function splitTopLevel(source, separator) {
  const parts = []
  let start = 0
  const depth = { '(': 0, '[': 0, '{': 0, '<': 0 }
  let quote = false
  let escaped = false
  for (let index = 0; index < source.length; index += 1) {
    const char = source[index]
    if (quote) {
      if (escaped) escaped = false
      else if (char === '\\') escaped = true
      else if (char === '"') quote = false
      continue
    }
    if (char === '"') {
      quote = true
      continue
    }
    if (Object.hasOwn(depth, char)) depth[char] += 1
    else if (char === ')' && depth['(']) depth['('] -= 1
    else if (char === ']' && depth['[']) depth['['] -= 1
    else if (char === '}' && depth['{']) depth['{'] -= 1
    else if (char === '>' && depth['<']) depth['<'] -= 1
    else if (char === separator && Object.values(depth).every((value) => value === 0)) {
      parts.push(source.slice(start, index).trim())
      start = index + 1
    }
  }
  if (source.slice(start).trim()) parts.push(source.slice(start).trim())
  return parts
}

function parseRustParameter(parameter) {
  const colon = splitTopLevel(parameter, ':')
  if (colon.length < 2) return null
  const name = colon[0].replace(/^mut\s+/, '').trim()
  if (!/^r?#?[A-Za-z_]\w*$/.test(name))
    throw new Error(`Could not parse Rust command argument: ${parameter}`)
  return { name: name.replace(/^r#/, ''), type: colon.slice(1).join(':').trim() }
}

function isTauriInjected(type) {
  return /(?:^|::)(?:AppHandle|Window|WebviewWindow)\b/.test(type) || /(?:^|::)State\s*</.test(type)
}

function toCamelCase(name) {
  return name.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())
}

export { parseRustParameter, rustCommandDefinitions, splitTopLevel, toCamelCase }
