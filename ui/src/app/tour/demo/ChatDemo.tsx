import { TopBar } from '../../shell/TopBar'
import { MessageReadingScope } from '../../../features/conversation/reading/MessageReadingScope'
import { TurnView } from '../../../features/conversation/messages/TurnView'
import { CoachPanelTabs } from '../../../features/conversation/coaching/CoachPanelTabs'
import { ConversationFeedbackCard } from '../../../features/conversation/coaching/ConversationFeedbackCard'
import { ComposerInput } from '../../../features/conversation/composer/ComposerInput'
import { AiStatus } from '../../../features/conversation/composer/AiStatus'
import { CHAT_FEEDBACK, CHAT_QUESTION, CHAT_READING_SCOPE, CHAT_REPLY } from './fixtures'

const noop = () => {}

/// The Chat surface, filled with a fixed exchange: the same top bar, message,
/// coach and composer components the real conversation uses, never a
/// redrawing of them. Nothing here sends, records or calls native code.
export function ChatDemo() {
  return <div className="demo-page">
    <TopBar />
    <div className="tour-demo-chat">
      <section className="chat">
        <div className="stream">
          <MessageReadingScope scope={CHAT_READING_SCOPE}>
            <TurnView editing={false} turn={{ id: 0, user: null, pendingText: '', assistant: CHAT_QUESTION }}
              reviewing={false} onAskCoach={noop} focused={false} ttsReady speaking={false} rtl={false} onBubbleTap={noop} onSpeak={noop} />
          </MessageReadingScope>
          <MessageReadingScope scope={CHAT_READING_SCOPE}>
            <TurnView editing={false} turn={{ id: 1, user: 'Ayer go al mercado.', assistant: CHAT_REPLY, pendingText: '', conversationFeedback: CHAT_FEEDBACK }}
              reviewing={false} onAskCoach={noop} focused={false} ttsReady speaking={false} rtl={false} onBubbleTap={noop} />
          </MessageReadingScope>
        </div>
        <div className="composer">
          <div className="composer-activity"><AiStatus transcribing={false} scheduling={false} turns={[]} synthesizing={false} beside={null} /></div>
          <ComposerInput input="" onInput={noop} available sending={false} recording={false} transcribing={false} autoSend
            targetLanguageTag="es" targetLanguageName="Español" onSend={noop} onToggleRecording={noop} onDiscardRecording={noop} />
        </div>
      </section>
      <section className="break">
        <CoachPanelTabs tab="coaching" onTab={noop} />
        <ConversationFeedbackCard feedback={CHAT_FEEDBACK} />
      </section>
    </div>
  </div>
}
