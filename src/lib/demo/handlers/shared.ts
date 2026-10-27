import { demoEntries } from '../data'

// Marker returned by a domain handler when a command does not belong to it.
export const notHandled = Symbol('notHandled')

// Shared mutable runtime state: file operations replace or mutate the listing.
export const state = { entries: demoEntries }
