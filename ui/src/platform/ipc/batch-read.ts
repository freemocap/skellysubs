/** Share reads requested in one synchronous burst. Clear before dispatch so a
 * later invalidation always starts a fresh read, even while this one is pending. */
export function batchRead<T>(read: () => Promise<T>): () => Promise<T> {
  let scheduled: Promise<T> | undefined
  return () => {
    scheduled ??= Promise.resolve().then(() => {
      scheduled = undefined
      return read()
    })
    return scheduled
  }
}
