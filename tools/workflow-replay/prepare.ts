/** Invoked after the capture-enabled Cargo build by workflow:prepare. */
import { copyFile, mkdir } from 'node:fs/promises'
import { fileURLToPath } from 'node:url'

if (process.platform !== 'win32') throw new Error('Workflow preparation currently requires Windows.')
const destination = new URL('../../.local/workflow-build/', import.meta.url)
await mkdir(destination, { recursive: true })
await copyFile(new URL('../../native/target/debug/skellyspeak.exe', import.meta.url), new URL('skellyspeak.exe', destination))
console.log(`Prepared isolated workflow executable: ${fileURLToPath(destination)}`)
