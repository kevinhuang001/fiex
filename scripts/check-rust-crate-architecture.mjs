#!/usr/bin/env node
import { execFileSync } from 'node:child_process'
import { relative, resolve } from 'node:path'
import { crateDependencyViolation, stronglyConnectedComponents } from './architecture-graph.mjs'

const ROOT = process.cwd()
const metadata = JSON.parse(
  execFileSync(
    'cargo',
    ['metadata', '--manifest-path', 'src-tauri/Cargo.toml', '--no-deps', '--format-version', '1'],
    { cwd: ROOT, encoding: 'utf8' },
  ),
)
const workspaceIds = new Set(metadata.workspace_members)
const packages = metadata.packages.filter((pkg) => workspaceIds.has(pkg.id))
const packagesByManifest = new Map(packages.map((pkg) => [resolve(pkg.manifest_path), pkg]))
const violations = []

function crateLayer(pkg) {
  const manifest = relative(ROOT, pkg.manifest_path).replaceAll('\\', '/')
  if (manifest === 'src-tauri/Cargo.toml') return 'app'
  if (manifest === 'sdk/rust/contracts/Cargo.toml') return 'contracts'
  if (manifest === 'sdk/rust/Cargo.toml') return 'sdk'
  const match = manifest.match(/^src-tauri\/crates\/(foundation|builtins|platform)\//)
  if (match) return match[1]
  throw new Error(`Workspace crate has no architecture layer: ${manifest}`)
}

const layers = new Map(packages.map((pkg) => [pkg.name, crateLayer(pkg)]))
const graph = new Map(packages.map((pkg) => [pkg.name, new Set()]))

for (const pkg of packages) {
  for (const dependency of pkg.dependencies) {
    if (!dependency.path) continue
    const target = packagesByManifest.get(resolve(dependency.path))
    if (!target) continue
    graph.get(pkg.name).add(target.name)
    const sourceLayer = layers.get(pkg.name)
    const targetLayer = layers.get(target.name)
    if (crateDependencyViolation(sourceLayer, targetLayer)) {
      violations.push(
        `${pkg.name} (${sourceLayer}) cannot depend on ${target.name} (${targetLayer})`,
      )
    }
  }
}

for (const cycle of stronglyConnectedComponents(graph)) {
  violations.push(`Rust crate dependency cycle: ${cycle.join(' -> ')}`)
}

if (violations.length) {
  console.error('Rust crate architecture check failed:')
  for (const violation of violations) console.error(`- ${violation}`)
  process.exit(1)
}

console.log(
  `Rust crate architecture check passed: ${packages.length} workspace crates; dependency layers and DAG are valid.`,
)
