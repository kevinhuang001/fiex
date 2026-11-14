import assert from 'node:assert/strict'
import test from 'node:test'
import {
  crateDependencyViolation,
  coreModuleBoundaryViolations,
  stronglyConnectedComponents,
} from './architecture-graph.mjs'

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

test('allows transfers and Core FS to depend on shared contracts without importing operations', () => {
  assert.deepEqual(
    coreModuleBoundaryViolations(
      'src-tauri/src/core/transfers/copy.rs',
      'use crate::core::operation_types::OperationResult;\n',
    ),
    [],
  )
})

test('rejects reverse imports from the operations façade outside test modules', () => {
  assert.deepEqual(
    coreModuleBoundaryViolations(
      'src-tauri/src/core/fs/transfer.rs',
      'use crate::core::operations::{OperationResult, check_cancelled};\n',
    ),
    ['src-tauri/src/core/fs/transfer.rs:1 depends on the operations façade'],
  )
  assert.deepEqual(
    coreModuleBoundaryViolations(
      'src-tauri/src/core/transfers/copy/tests.rs',
      'use crate::core::operations::Operations;\n',
    ),
    [],
  )
})
