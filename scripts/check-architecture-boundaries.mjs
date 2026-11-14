#!/usr/bin/env node
import { readFileSync, readdirSync } from 'node:fs'
import path from 'node:path'
import ts from 'typescript'
import { featureDependencyViolation } from './architecture-boundaries.mjs'
import { stronglyConnectedComponents } from './architecture-graph.mjs'

const ROOT = process.cwd()
const SOURCE_ROOT = path.join(ROOT, 'src')
const FEATURE_ROOT = path.join(SOURCE_ROOT, 'features')
const SOURCE_EXTENSIONS = new Set(['.ts', '.tsx'])
const CYCLE_BASELINE = path.join(ROOT, 'scripts', 'architecture-cycle-baseline.json')
const violations = []

function sourceFiles(directory, files = []) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name)
    if (entry.isDirectory()) sourceFiles(file, files)
    else if (SOURCE_EXTENSIONS.has(path.extname(file))) files.push(file)
  }
  return files
}

function importSpecifiers(sourceFile) {
  const specifiers = []
  function visit(node) {
    if ((ts.isImportDeclaration(node) || ts.isExportDeclaration(node)) && node.moduleSpecifier) {
      if (ts.isStringLiteral(node.moduleSpecifier)) specifiers.push(node.moduleSpecifier.text)
    } else if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        (ts.isIdentifier(node.expression) && node.expression.text === 'require')) &&
      node.arguments.length === 1 &&
      ts.isStringLiteral(node.arguments[0])
    ) {
      specifiers.push(node.arguments[0].text)
    } else if (
      ts.isImportTypeNode(node) &&
      ts.isLiteralTypeNode(node.argument) &&
      ts.isStringLiteral(node.argument.literal)
    ) {
      specifiers.push(node.argument.literal.text)
    }
    ts.forEachChild(node, visit)
  }
  visit(sourceFile)
  return specifiers
}

function dependencyDeclarations(sourceFile) {
  const declarations = []
  function visit(node) {
    if (ts.isImportDeclaration(node) && ts.isStringLiteral(node.moduleSpecifier)) {
      const clause = node.importClause
      const runtime =
        !clause ||
        (!clause.isTypeOnly &&
          (!!clause.name ||
            !clause.namedBindings ||
            ts.isNamespaceImport(clause.namedBindings) ||
            clause.namedBindings.elements.some((element) => !element.isTypeOnly)))
      declarations.push({ specifier: node.moduleSpecifier.text, runtime })
    } else if (
      ts.isExportDeclaration(node) &&
      node.moduleSpecifier &&
      ts.isStringLiteral(node.moduleSpecifier)
    ) {
      const runtime =
        !node.isTypeOnly &&
        (!node.exportClause ||
          !ts.isNamedExports(node.exportClause) ||
          node.exportClause.elements.some((element) => !element.isTypeOnly))
      declarations.push({ specifier: node.moduleSpecifier.text, runtime })
    } else if (
      ts.isCallExpression(node) &&
      (node.expression.kind === ts.SyntaxKind.ImportKeyword ||
        (ts.isIdentifier(node.expression) && node.expression.text === 'require')) &&
      node.arguments.length === 1 &&
      ts.isStringLiteral(node.arguments[0])
    ) {
      declarations.push({ specifier: node.arguments[0].text, runtime: true })
    } else if (
      ts.isImportTypeNode(node) &&
      ts.isLiteralTypeNode(node.argument) &&
      ts.isStringLiteral(node.argument.literal)
    ) {
      declarations.push({ specifier: node.argument.literal.text, runtime: false })
    }
    ts.forEachChild(node, visit)
  }
  visit(sourceFile)
  return declarations
}

function relativeSourcePath(file) {
  return path.relative(ROOT, file).replaceAll('\\', '/')
}

const config = ts.readConfigFile(path.join(ROOT, 'tsconfig.json'), ts.sys.readFile)
if (config.error) {
  throw new Error(ts.flattenDiagnosticMessageText(config.error.messageText, '\n'))
}
const compilerOptions = ts.parseJsonConfigFileContent(config.config, ts.sys, ROOT).options
const allSourceFiles = sourceFiles(SOURCE_ROOT).filter((file) => !file.endsWith('.d.ts'))
const runtimeGraph = new Map(allSourceFiles.map((file) => [relativeSourcePath(file), new Set()]))
const typeGraph = new Map(allSourceFiles.map((file) => [relativeSourcePath(file), new Set()]))

for (const file of allSourceFiles) {
  const sourceFile = ts.createSourceFile(
    file,
    readFileSync(file, 'utf8'),
    ts.ScriptTarget.Latest,
    true,
    file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  )
  const sourcePath = relativeSourcePath(file)
  for (const declaration of dependencyDeclarations(sourceFile)) {
    const resolved = ts.resolveModuleName(declaration.specifier, file, compilerOptions, ts.sys)
      .resolvedModule?.resolvedFileName
    if (!resolved) continue
    const targetPath = relativeSourcePath(resolved)
    if (!typeGraph.has(targetPath)) continue
    typeGraph.get(sourcePath).add(targetPath)
    if (declaration.runtime) runtimeGraph.get(sourcePath).add(targetPath)
  }
}

for (const file of sourceFiles(FEATURE_ROOT)) {
  const sourcePath = relativeSourcePath(file)
  const sourceFile = ts.createSourceFile(
    file,
    readFileSync(file, 'utf8'),
    ts.ScriptTarget.Latest,
    true,
    file.endsWith('.tsx') ? ts.ScriptKind.TSX : ts.ScriptKind.TS,
  )
  for (const specifier of importSpecifiers(sourceFile)) {
    const resolved = ts.resolveModuleName(specifier, file, compilerOptions, ts.sys).resolvedModule
      ?.resolvedFileName
    if (!resolved) continue
    const targetPath = relativeSourcePath(resolved)
    if (featureDependencyViolation(sourcePath, targetPath)) {
      violations.push(`${sourcePath}: feature code cannot import UI or hook module ${specifier}`)
    }
  }
}

const runtimeCycles = stronglyConnectedComponents(runtimeGraph)
if (runtimeCycles.length) {
  violations.push(...runtimeCycles.map((cycle) => `runtime import cycle: ${cycle.join(' -> ')}`))
}

const typeCycles = stronglyConnectedComponents(typeGraph)
const knownTypeCycles = JSON.parse(readFileSync(CYCLE_BASELINE, 'utf8'))
for (const cycle of typeCycles) {
  const known = knownTypeCycles.some((baseline) => cycle.every((file) => baseline.includes(file)))
  if (!known) violations.push(`new type-level import cycle: ${cycle.join(' -> ')}`)
}

if (violations.length) {
  console.error('Architecture boundary check failed:')
  for (const violation of violations) console.error(`- ${violation}`)
  process.exit(1)
}

console.log(
  `Architecture boundary check passed: ${sourceFiles(FEATURE_ROOT).length} feature files checked; ${allSourceFiles.length} source modules scanned, no runtime cycles, ${typeCycles.length} known type-level cycle(s).`,
)
