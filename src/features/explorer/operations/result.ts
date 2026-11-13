import { t } from '@/i18n'
import type { OperationResult } from '@/lib/types'
import type { HandleResult } from './types'

/** Reports a completed operation, turning partial failures into an error the
 *  pane can show. */
export function createResultHandler(notify: (message: string) => void): HandleResult {
  return (result: OperationResult) => {
    if (result.errors.length)
      throw new Error(t('{0} 项已完成。{1}', result.paths.length, result.errors.join('\n')))
    notify(
      result.skipped
        ? t('{0} 项操作完成，已跳过 {1} 项', result.paths.length, result.skipped)
        : t('{0} 项操作完成', result.paths.length),
    )
  }
}
