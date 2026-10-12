import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { FriendlyReason } from '../../../components/feedback/FriendlyError'
import { useRef, useState } from 'react'
import type { ReplyState } from '../../../domain/conversation/reply-state'
import { nativeError } from '../../../platform/ipc/workspace'
import { ActivityIndicator } from '../../../components/feedback/ActivityIndicator'
import { useI18n } from '../../../components/localization/i18n'
import { useReadingPreferences } from '../../../components/reading/ReadingPreferences'
import { PendingBubble, ReceivedText } from './PendingBubble'

/** The partner's reply before it lands. Only first admission requests the
 * `arriving` fade; the saved turn's existing placeholder leaves it off. */
export function ReplyStatus({ reply, stream, retainedText, visibleText, rtl, onControl, onActivity, arriving = false }: {
  reply?: ReplyState; stream?: { text: string } | null; retainedText?: string | null; rtl?: boolean
  visibleText?: string
  onControl?: (control: 'retry' | 'resume') => Promise<void>; onActivity?: () => void; arriving?: boolean
}) {
  const tr = useI18n()
  const { autoTranslate, alwaysRomanize, alwaysPronunciation } = useReadingPreferences()
  const [pending, setPending] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const locked = useRef(false)
  const state = reply?.state ?? 'unavailable'
  const received = stream?.text || retainedText || null
  if (state === 'pending') {
    const summary = <ActivityIndicator announce={false} label={received ? tr('Receiving reply…') : tr('Replying…')} />
    return <div className="partner-turn"><span className="partner-reaction-slot" aria-hidden="true" />
      <PendingBubble side="bot" text={received} visibleText={visibleText} streaming rtl={rtl} activity={summary} arriving={arriving}
        aids={autoTranslate || alwaysRomanize || alwaysPronunciation} translationSlot={autoTranslate} />
    </div>
  }
  const label = state === 'failed' ? tr('Partner reply failed.') : state === 'unknown' ? tr('Partner reply outcome is unknown. Retrying may repeat provider work and charges.') : state === 'cancelled' ? tr('Partner reply was cancelled. Send a new message to continue.') : state === 'held' ? tr('Partner reply is held.') : state === 'paused' ? tr('Partner reply is paused.') : tr('Partner reply is unavailable.')
  return <div className="partner-turn"><span className="partner-reaction-slot" aria-hidden="true" /><div className="msg chat-message bot" data-reply-state={state}>
    {received && <ReceivedText text={received} streaming={false} rtl={rtl} />}
    <p role={state === 'failed' || state === 'unknown' || state === 'unavailable' ? 'alert' : 'status'}>{label}</p>
    {reply?.error && <FriendlyReason error={reply.error} />}
    {reply?.control && onControl && <button type="button" disabled={pending} onClick={() => {
      if (locked.current) return
      locked.current = true; setPending(true); setError(null)
      void onControl(reply.control!).catch(reason => setError(nativeError(reason))).finally(() => { locked.current = false; setPending(false) })
    }}>{reply.control === 'retry' ? tr('Retry exchange') : tr('Resume exchange')}</button>}
    {onActivity && <button type="button" onClick={onActivity}>{tr('Open AI activity')}</button>}
    {error && <ErrorNotice as="p" error={error}>{error}</ErrorNotice>}
  </div></div>
}
