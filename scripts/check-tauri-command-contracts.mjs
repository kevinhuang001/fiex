#!/usr/bin/env node
import { readdirSync, readFileSync } from 'node:fs'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import ts from 'typescript'
import { rustCommandDefinitions } from './tauri-command-parser.mjs'

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..')

function readRustCommands(directory) {
  const commands = []
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) commands.push(...readRustCommands(path))
    else if (entry.name.endsWith('.rs'))
      commands.push(...rustCommandDefinitions(readFileSync(path, 'utf8')))
  }
  return commands
}

function interfaceProperties(sourceFile, interfaceName) {
  const declaration = sourceFile.statements.find(
    (statement) => ts.isInterfaceDeclaration(statement) && statement.name.text === interfaceName,
  )
  if (!declaration) throw new Error(`Could not find ${interfaceName} in ${sourceFile.fileName}`)
  return declaration.members
    .filter((member) => ts.isPropertySignature(member) && member.name)
    .map((member) => [member.name.getText(sourceFile).replace(/^['"]|['"]$/g, ''), member])
}

function readTypeScriptContracts() {
  const argsPath = join(root, 'src/lib/tauriCommandArgs.ts')
  const resultsPath = join(root, 'src/lib/tauriCommandMap.ts')
  const configPath = join(root, 'tsconfig.json')
  const config = ts.readConfigFile(configPath, ts.sys.readFile)
  if (config.error) throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'))
  const parsed = ts.parseJsonConfigFileContent(config.config, ts.sys, root)
  const program = ts.createProgram([argsPath, resultsPath], parsed.options)
  const checker = program.getTypeChecker()
  const argsFile = program.getSourceFile(argsPath)
  const resultsFile = program.getSourceFile(resultsPath)
  if (!argsFile || !resultsFile) throw new Error('Could not load Tauri TypeScript contract files')
  const args = new Map(
    interfaceProperties(argsFile, 'TauriCommandArgs').map(([name, member]) => {
      const type = checker.getTypeAtLocation(member.type ?? member)
      const keys = checker
        .getPropertiesOfType(type)
        .map((property) => property.getName())
        .filter((key) => key !== '__noArgs')
      return [name, keys]
    }),
  )
  const results = interfaceProperties(resultsFile, 'TauriCommandResults').map(([name]) => name)
  return { args, results }
}

function readDecoderCommandNames() {
  const validationDir = join(root, 'src/lib/ipcValidation')
  const files = [join(root, 'src/lib/ipcValidation.ts')]
  for (const entry of readdirSync(validationDir, { withFileTypes: true })) {
    if (entry.isFile() && entry.name.endsWith('.ts') && !entry.name.endsWith('.test.ts'))
      files.push(join(validationDir, entry.name))
  }
  const names = new Set()
  for (const path of files) {
    const source = readFileSync(path, 'utf8')
    const ast = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true)
    const visit = (node) => {
      if (
        ts.isBinaryExpression(node) &&
        node.operatorToken.kind === ts.SyntaxKind.EqualsEqualsEqualsToken
      ) {
        const sides = [node.left, node.right]
        if (sides.some((side) => ts.isIdentifier(side) && side.text === 'command')) {
          const literal = sides.find(ts.isStringLiteral)
          if (literal) names.add(literal.text)
        }
      }
      if (ts.isVariableDeclaration(node) && ts.isIdentifier(node.name)) {
        if (
          node.name.text === 'SIMPLE_RESULT_KINDS' &&
          node.initializer &&
          ts.isObjectLiteralExpression(node.initializer)
        ) {
          for (const property of node.initializer.properties) {
            if (
              ts.isPropertyAssignment(property) &&
              (ts.isIdentifier(property.name) || ts.isStringLiteral(property.name))
            )
              names.add(property.name.text)
          }
        }
        if (
          node.name.text === 'STRING_ARRAY_COMMANDS' &&
          node.initializer &&
          ts.isNewExpression(node.initializer) &&
          node.initializer.arguments?.[0] &&
          ts.isArrayLiteralExpression(node.initializer.arguments[0])
        ) {
          for (const element of node.initializer.arguments[0].elements)
            if (ts.isStringLiteral(element)) names.add(element.text)
        }
      }
      ts.forEachChild(node, visit)
    }
    visit(ast)
  }
  return names
}

function assertSameNames(label, expected, actual) {
  const expectedSet = new Set(expected)
  const actualSet = new Set(actual)
  const missing = [...expectedSet].filter((name) => !actualSet.has(name)).sort()
  const stale = [...actualSet].filter((name) => !expectedSet.has(name)).sort()
  const expectedDuplicates = expected.length - expectedSet.size
  const actualDuplicates = actual.length - actualSet.size
  if (missing.length || stale.length || expectedDuplicates || actualDuplicates) {
    throw new Error(
      `${label} differs. Missing: ${missing.join(', ') || 'none'}; ` +
        `stale: ${stale.join(', ') || 'none'}; duplicates: ${expectedDuplicates}/${actualDuplicates}`,
    )
  }
}

export function runContractCheck() {
  const handlerSource = readFileSync(join(root, 'src-tauri/src/desktop/mod.rs'), 'utf8')
  const handlerBody = handlerSource.match(/generate_handler!\[([\s\S]*?)\]/)?.[1]
  if (!handlerBody) throw new Error('Could not find the Tauri command handler list')
  const registered = handlerBody
    .split(',')
    .map((entry) => entry.trim().split('::').at(-1))
    .filter(Boolean)
  assertSameNames(
    'Registered command handlers vs Rust #[tauri::command] functions',
    registered,
    readRustCommands(join(root, 'src-tauri/src/desktop')).map(({ name }) => name),
  )

  const { args, results } = readTypeScriptContracts()
  assertSameNames('Rust handlers vs TauriCommandArgs', registered, [...args.keys()])
  assertSameNames('Rust handlers vs TauriCommandResults', registered, results)
  const rustCommands = readRustCommands(join(root, 'src-tauri/src/desktop'))
  for (const { name, args: rustArgs } of rustCommands) {
    const mapped = args.get(name)
    if (!mapped) continue
    assertSameNames(`${name} argument names`, rustArgs, mapped)
  }

  assertSameNames('TauriCommandResults vs runtime result decoders', results, [
    ...readDecoderCommandNames(),
  ])
  console.log(
    `Tauri command contracts cover ${registered.length} handlers, argument lists, results, and runtime decoders.`,
  )
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href)
  runContractCheck()
