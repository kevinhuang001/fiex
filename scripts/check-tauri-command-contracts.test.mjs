import assert from 'node:assert/strict'
import test from 'node:test'
import {
  parseRustParameter,
  rustCommandDefinitions,
  splitTopLevel,
  toCamelCase,
} from './tauri-command-parser.mjs'

test('converts Rust snake case command argument names to Tauri camel case', () => {
  assert.equal(toCamelCase('on_snapshot'), 'onSnapshot')
  assert.equal(toCamelCase('path'), 'path')
})

test('splits nested Rust generic and tuple types only at top-level commas', () => {
  assert.deepEqual(splitTopLevel('path: String, data: Option<Vec<(u8, u8)>>', ','), [
    'path: String',
    'data: Option<Vec<(u8, u8)>>',
  ])
  assert.deepEqual(parseRustParameter("mut state: tauri::State<'_, Store>"), {
    name: 'state',
    type: "tauri::State<'_, Store>",
  })
})

test('extracts command arguments, ignores injected Tauri state, and skips doc comments', () => {
  const commands = rustCommandDefinitions(`
    //! #[tauri::command]
    #[tauri::command(rename_all = "camelCase")]
    pub(crate) async fn sample(
      app: tauri::AppHandle,
      state: tauri::State<'_, Store>,
      on_snapshot: Channel<DirectoryViewUpdate>,
      values: Option<Vec<(u8, u8)>>,
    ) -> Result<(), String> { Ok(()) }
    #[tauri::command(rename_all = "snake_case")]
    fn snake_case_mode(on_snapshot: Channel<String>) {}
  `)

  assert.deepEqual(commands, [
    { name: 'sample', args: ['onSnapshot', 'values'] },
    { name: 'snake_case_mode', args: ['on_snapshot'] },
  ])
})
