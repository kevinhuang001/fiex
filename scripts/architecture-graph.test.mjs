import assert from 'node:assert/strict'
import test from 'node:test'
import { crateDependencyViolation, stronglyConnectedComponents } from './architecture-graph.mjs'

test('reports only cyclic module components, including self-imports', () => {
  const graph = new Map([
    ['a', new Set(['b'])],
    ['b', new Set(['a', 'c'])],
    ['c', new Set()],
    ['self', new Set(['self'])],
  ])

  assert.deepEqual(stronglyConnectedComponents(graph), [['a', 'b'], ['self']])
})

test('accepts dependencies toward stable Rust layers', () => {
  assert.equal(crateDependencyViolation('builtins', 'foundation'), false)
  assert.equal(crateDependencyViolation('platform', 'foundation'), false)
  assert.equal(crateDependencyViolation('app', 'builtins'), false)
})

test('rejects dependencies that point back toward higher Rust layers', () => {
  assert.equal(crateDependencyViolation('foundation', 'builtins'), true)
  assert.equal(crateDependencyViolation('platform', 'builtins'), true)
  assert.equal(crateDependencyViolation('contracts', 'app'), true)
})
