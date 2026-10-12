/** Authored presentation preferences only. Access tokens and arbitrary storage
 * are not part of the workflow browser snapshot. */
export function presentationStorage(entries: Record<string, string>): Record<string, string> {
  const fixed = new Set(['skellyspeak.practice-view', 'skellyspeak_dev_h', 'skellyspeak_ai_inspector_width', 'skellyspeak_skills_intro', 'skellyspeak_break', 'skellyspeak_practice_starter', 'skellyspeak_cards_panel', 'skellyspeak_skill_chart_type', 'skellyspeak_skill_chart_scale', 'skellyspeak_skill_chart_shown'])
  const result: Record<string, string> = {}
  for (const [key, value] of Object.entries(entries)) {
    if (!fixed.has(key) && !/^skellyspeak_recorder_(chat|practice)_(pad|time)$/.test(key) && !/^skellyspeak_pane_[a-z_-]{1,100}$/.test(key) && !/^skellyspeak_drill_phrase_[a-z_-]{1,100}$/.test(key)) continue
    if (typeof value !== 'string' || value.length > 1024) throw new Error('Invalid presentation preference.')
    result[key] = value
  }
  return result
}
