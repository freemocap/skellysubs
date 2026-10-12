import { createHash } from 'node:crypto'
import { readFile, readdir, lstat } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

/** Includes every served frontend asset and the executable, not just index.html. */
export async function buildIdentity() {
  const root = fileURLToPath(new URL('../../', import.meta.url))
  const hash = createHash('sha256')
  hash.update(await readFile(join(root,'.local/workflow-build/skellyspeak.exe')).catch(() => {
    throw new Error('Prepared workflow executable is missing. Run npm run workflow:prepare.')
  }))
  async function visit(directory:string, prefix:string) {
    for (const name of (await readdir(directory)).sort()) {
      const path=join(directory,name)
      const info=await lstat(path)
      if (info.isSymbolicLink()) throw new Error('Build assets must not contain links.')
      if (info.isDirectory()) await visit(path,`${prefix}${name}/`)
      else if (info.isFile()) { hash.update(`${prefix}${name}\0${info.size}\0`);hash.update(await readFile(path)) }
      else throw new Error('Unsupported build asset.')
    }
  }
  await visit(join(root,'ui/dist'),'')
  return hash.digest('hex')
}
