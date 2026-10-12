import { retryMessageWork } from '../../../platform/ipc/retry-ai'
import { useContext } from 'react'
import { SkillEvidenceContext } from '../../../state/learning/useSkillEvidence'
import { PracticeContext } from '../session/PracticeContext'
import { skillIndex } from '../../../domain/learning/catalog/skill-index'
import { SkillGuide } from '../../../components/learning/SkillGuide'
import { TargetPhrase } from '../../../components/reading/TargetPhrase'
import { ReadingLanguageScope } from '../../../components/reading/ReadingLanguageScope'
import { ErrorNotice } from '../../../components/feedback/ErrorNotice'
import { useI18n } from '../../../components/localization/i18n'

/** Saved learner evidence only; reading a partner's explanation earns no credit. */
export function MessageSkillAnalysis({ messageId, source, conversationId, graphTarget }: { graphTarget?: Record<string, string>; messageId: number; source: string; conversationId?: string }) {
  const tr = useI18n()
  const { snapshot, error } = useContext(SkillEvidenceContext)
  const practice = useContext(PracticeContext)
  const chatId = conversationId ?? practice?.chatId
  if (error) return <ErrorNotice error={error}>{error}</ErrorNotice>
  if (!snapshot || !chatId) return null
  const index = skillIndex(snapshot)
  const records = (index.messages.get(JSON.stringify([chatId, messageId])) ?? []).filter(record => record.source === source && record.status === 'complete' && !index.excluded.has(record.attempt_id))
  const rows = records.flatMap(record => (record.assessment?.judgments ?? []).filter(judgment => ['direct', 'contextual'].includes(judgment.presence ?? '')).map(judgment => ({ record, judgment, skill: index.catalog.node(judgment.skill_id) })))
  if (!rows.length) return null
  return <section className="xp-evidence-report message-skill-credit" aria-label={tr('Skills')}>
    <details><summary>{tr('Skills')} <span className="message-skill-credit-count">{tr.number(rows.length)}</span></summary>
    {rows.map(({ record, judgment, skill }) => <article {...graphTarget} className="practice-credit" key={`${record.attempt_id}:${skill.id}`}>
      <header><strong>{tr(skill.label)}</strong></header>
      <ReadingLanguageScope language={record.target} variety={record.variety} explanation={record.native}>
        {(judgment.quotes.length ? judgment.quotes : [source]).map((quote, i) => <blockquote key={i}><TargetPhrase text={quote} /></blockquote>)}
      </ReadingLanguageScope>
      <p>{tr(skill.criterion)}</p>
      {record.attribution_error && <ErrorNotice onRetry={() => retryMessageWork(record.chat_id, record.message_id)} error={record.attribution_error}>{record.attribution_error}</ErrorNotice>}
      <SkillGuide initiallyOpen={false} snapshot={snapshot} skillId={skill.id} active={record.variety ?? undefined} />
    </article>)}
    </details>
  </section>
}
