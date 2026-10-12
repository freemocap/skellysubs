import type { GuideAction } from '../../../components/learning/GuideActions'
import { useSkillEvidenceStore } from '../../../state/learning/skill-evidence'
import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import type { ChatSummary, Settings, StoredTurn } from '../../../types'
import type { Snapshot } from '../../../generated/contracts'
import { conversationTurns } from '../../../domain/conversation/conversation-view'
import { replyState } from '../../../domain/conversation/reply-state'
import { executeAction, nativeError, readWorkspace, selectedConversation } from '../../../platform/ipc/workspace'
import { unreportedInput, type InputEvidence } from '../../../domain/learning/evidence/skills'
import { reportFault } from '../../../platform/diagnostics/faults'
import { useConversationSnapshot } from './useConversationSnapshot'

export type Turn = StoredTurn & { pendingText: string }
interface Options {
  settings: Settings | null
  setHistoryOpen: (open: boolean) => void
  resetView: () => void
}

/** Native snapshots own messages. Mounting observes; only explicit commands create work. */
export function useConversation({ settings, setHistoryOpen, resetView }: Options) {
  const [chats, setChats] = useState<ChatSummary[]>([])
  const [currentChatId, setCurrentChatId] = useState<string | null>(null)
  const [openingFailed, setOpeningFailed] = useState(false)
  const observation = useConversationSnapshot(currentChatId)
  const { snapshot } = observation
  const turns = useMemo(() => snapshot ? conversationTurns(snapshot).map(t => ({ ...t, pendingText: '' })) : [], [snapshot?.messages, snapshot?.turns, snapshot?.connection])
  useEffect(() => { if (snapshot) useSkillEvidenceStore.getState().reload() }, [snapshot?.contentRevision, currentChatId])
  const turnsRef = useRef<Turn[]>([])
  turnsRef.current = turns
  const chatIdRef = useRef<{ target: string; native: string; id: string } | null>(null)
  const directoryRef = useRef<Snapshot | null>(null)
  const actionPending = useRef(false)
  const resetRef = useRef(resetView)
  resetRef.current = resetView
  const target = settings?.target_language
  const native = settings?.native_language

  const refresh = useCallback(async () => {
    const directory = await readWorkspace()
    directoryRef.current = directory
    setChats(directory.conversations.filter(c => !c.archived && (!target || c.languageId === target))
      .sort((a, b) => b.lastUsed - a.lastUsed)
      .map(c => ({ id: c.id, title: c.title, updated_at: Math.floor(c.lastUsed / 1000) })))
    return directory
  }, [target])

  useEffect(() => {
    let disposed = false
    chatIdRef.current = null
    setCurrentChatId(null)
    if (!target || !native) return
    void refresh().then(directory => {
      if (disposed) return
      const selected = selectedConversation(directory, target)
      if (!selected) throw new Error('No conversation exists for this language. Start a new conversation.')
      chatIdRef.current = { id: selected.id, target, native }
      setCurrentChatId(selected.id)
      setOpeningFailed(false)
    }).catch(error => {
      if (!disposed) { setOpeningFailed(true); reportFault('Opening conversation', nativeError(error)) }
    })
    return () => { disposed = true }
  }, [target, native, refresh])

  const openChat = useCallback(async (id: string) => {
    const directory = await refresh()
    const conversation = directory.conversations.find(c => c.id === id && !c.archived)
    if (!conversation) throw new Error('Conversation is unavailable.')
    await executeAction(directory, { kind: 'openConversation', conversationId: id })
    resetRef.current()
    chatIdRef.current = { id, target: conversation.languageId, native: conversation.settings.explanationLanguage }
    setCurrentChatId(id)
    setHistoryOpen(false)
  }, [refresh, setHistoryOpen])

  const startNew = useCallback(async () => {
    if (actionPending.current) return
    actionPending.current = true
    try {
      const directory = await refresh()
      const owner = directory.conversations.find(c => c.id === chatIdRef.current?.id)
      const receipt = await executeAction(directory, owner
        ? { kind: 'createConversation', contactId: owner.contactId, title: 'Conversation' }
        : { kind: 'startChat', languageId: target ?? 'spanish' })
      if (!receipt.entityId) throw new Error('No new conversation identity was returned.')
      await openChat(receipt.entityId)
      await refresh()
    } catch (error) { reportFault('Starting conversation', nativeError(error)) }
    finally { actionPending.current = false }
  }, [openChat, refresh, target])

  const startFromPhrase = useCallback(async (sourceMessageId: string, phrase: string) => {
    if (actionPending.current) return
    actionPending.current = true
    const owner = chatIdRef.current
    try {
      const directory = await refresh()
      if (!owner || chatIdRef.current?.id !== owner.id) throw new Error('The conversation changed before starting.')
      const conversation = directory.conversations.find(c => c.id === owner.id)
      if (!conversation) throw new Error('Conversation is unavailable.')
      const receipt = await executeAction(directory, { kind: 'startPhraseConversation', sourceMessageId, phrase, contactId: conversation.contactId, expectedRevision: directory.revision })
      if (chatIdRef.current?.id === owner.id) await openChat(receipt.entityId)
      await refresh()
    } finally { actionPending.current = false }
  }, [openChat, refresh])

  const startFromSkill = useCallback(async (language: string, variety: string, skillId: string, subskillId: string | null = null) => {
    if (actionPending.current) throw new Error('A conversation is already being started.')
    actionPending.current = true
    const owner = chatIdRef.current
    try {
      const directory = await refresh()
      if (!owner || chatIdRef.current?.id !== owner.id) throw new Error('The conversation changed before starting.')
      const receipt = await executeAction(directory, { kind: 'startSkillConversation', sourceConversationId: owner.id, language, variety, skillId, subskillId, expectedRevision: directory.revision })
      if (chatIdRef.current?.id === owner.id) await openChat(receipt.entityId)
      await refresh()
    } finally { actionPending.current = false }
  }, [openChat, refresh])

  const runGuideAction = useCallback(async (action: GuideAction) => {
    if (actionPending.current) throw new Error('A conversation action is already running.')
    actionPending.current = true
    const owner = chatIdRef.current
    try {
      const directory = await refresh()
      if (!owner || chatIdRef.current?.id !== owner.id) throw new Error('The conversation changed before continuing.')
      const conversation = directory.conversations.find(c => c.id === owner.id)
      if (!conversation) throw new Error('Conversation is unavailable.')
      const receipt = await executeAction(directory, action.kind === 'coach'
        ? { kind: 'askGuideCoach', conversationId: owner.id, text: action.text, guide: action.guide, focus: action.focus, expectedRevision: conversation.revision }
         : action.kind === 'skill'
          ? { kind: 'startSkillConversation', sourceConversationId: owner.id, language: action.guide.language, variety: action.guide.variety, skillId: action.guide.skillId, subskillId: action.subskillId, expectedRevision: directory.revision }
          : { kind: 'startGuideConversation', sourceConversationId: owner.id, guide: action.guide, example: action.example, phrase: action.phrase, expectedRevision: directory.revision })
      if (action.kind !== 'coach' && chatIdRef.current?.id === owner.id) await openChat(receipt.entityId)
      await refresh()
    } finally { actionPending.current = false }
  }, [openChat, refresh])

  const removeChat = useCallback(async (id: string) => {
    const directory = await refresh()
    const conversation = directory.conversations.find(c => c.id === id)
    if (!conversation) throw new Error('Conversation is unavailable.')
    await executeAction(directory, { kind: 'deleteConversation', conversationId: id, expectedRevision: conversation.revision })
    const next = await refresh()
    if (currentChatId === id) {
      chatIdRef.current = null
      setCurrentChatId(null)
      resetRef.current()
      const selected = selectedConversation(next, target)
      if (selected) await openChat(selected.id)
    }
  }, [currentChatId, openChat, refresh, target])

  const sendMessage = useCallback(async (text: string, expectedConversationId?: string | null, input: InputEvidence = unreportedInput()) => {
    const owner = chatIdRef.current
    if (!owner) throw new Error('No conversation is open.')
    if (expectedConversationId !== undefined && owner.id !== expectedConversationId) throw new Error('The conversation changed before sending.')
    const directory = await readWorkspace()
    const conversation = directory.conversations.find(c => c.id === owner.id)
    if (!conversation) throw new Error('Conversation is unavailable.')
    return executeAction(directory, { kind: 'sendMessage', conversationId: owner.id, expectedRevision: conversation.revision, text, input })
  }, [])

  return { ...observation, turns, turnsRef, chats, currentChatId, openingFailed, chatIdRef, openChat, startNew, startFromPhrase, startFromSkill, runGuideAction, removeChat, sendMessage,
    snapshot: snapshot?.conversationId === currentChatId ? snapshot : null,
    snapshotRevision: snapshot?.revision ?? -1,
    pendingReply: snapshot?.turns.some(t => t.state === 'pending') ?? false,
    replyActive: snapshot?.turns.some(turn => replyState(turn, snapshot).state === 'pending') ?? false,
    flush: async () => { if (!chatIdRef.current) throw new Error('No conversation is open.') },
  }
}
