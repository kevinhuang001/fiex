import type { OperationResult } from '@/lib/types'
import type { HandleResult } from './types'

/** Reports a completed operation, turning partial failures into an error the
 *  pane can show. */
export function createResultHandler(notify: (message: string) => void): HandleResult {
  return (result: OperationResult) => {
    if (result.errors.length)
      throw new Error(`${result.paths.length} 项已完成。${result.errors.join('\n')}`)
    notify(
      result.skipped
        ? `${result.paths.length} 项操作完成，已跳过 ${result.skipped} 项`
        : `${result.paths.length} 项操作完成`,
    )
  }
}
