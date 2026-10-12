/** Read-only CDP discovery. Does not start, reload, attach to, or drive a window. */
export function loopbackEndpoint(value: string): URL {
  const url = new URL(value)
  if (url.protocol !== 'http:' || !['127.0.0.1', '[::1]'].includes(url.hostname) || !url.port || url.username || url.password || url.pathname !== '/' || url.search || url.hash) {
    throw new Error('Supply an explicit loopback HTTP origin with a port.')
  }
  return url
}

export async function discoverDesktop(value: string, request: typeof fetch = fetch): Promise<{ browser: string; pages: number }> {
  const origin = loopbackEndpoint(value)
  const read = async (path: string): Promise<unknown> => {
    const response = await request(new URL(path, origin), { redirect: 'error', signal: AbortSignal.timeout(3000) })
    if (!response.ok) throw new Error('CDP discovery failed.')
    const reader = response.body?.getReader()
    if (!reader) throw new Error('CDP discovery returned no body.')
    const chunks: Uint8Array[] = []
    let size = 0
    try {
      while (true) {
        const next = await reader.read()
        if (next.done) break
        size += next.value.byteLength
        if (size > 64 * 1024) throw new Error('CDP discovery exceeded its size limit.')
        chunks.push(next.value)
      }
    } finally { await reader.cancel(); reader.releaseLock() }
    return JSON.parse(Buffer.concat(chunks).toString('utf8')) as unknown
  }
  const version = await read('/json/version')
  const pages = await read('/json/list')
  if (!version || typeof version !== 'object' || !('Browser' in version) || typeof version.Browser !== 'string' || !Array.isArray(pages)) throw new Error('Invalid CDP discovery response.')
  // Do not persist target URLs/titles: they may contain learner content.
  return { browser: version.Browser, pages: pages.filter(page => page && typeof page === 'object' && page.type === 'page').length }
}

if (import.meta.main) {
  const endpoint = process.argv[2]
  if (!endpoint) {
    let playwrightInstalled = false
    try { import.meta.resolve('playwright'); playwrightInstalled = true } catch { /* Report availability without installation. */ }
    console.log(JSON.stringify({ platform: process.platform, playwrightInstalled, desktopAttachment: 'unverified', capture: 'not-connected' }))
  } else {
    try { console.log(JSON.stringify(await discoverDesktop(endpoint))) }
    catch { console.error('Desktop discovery failed; no application changes were made.'); process.exitCode = 1 }
  }
}
