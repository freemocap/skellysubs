import { create } from 'zustand'
import { getSkillEvidence, saveSkillProfile } from '../../platform/ipc/skill-evidence'
import type { ProfileChoices, SkillSnapshot } from '../../domain/learning/evidence/skills'

/// Skill evidence, as one value the whole app reads.
///
/// Evidence moves when the conversation advances, so whoever sees the
/// conversation change asks for a reload. Callers are direct: nothing has to be
/// listening for the change, and nothing has to unsubscribe.

export interface SkillEvidenceState {
  /** Desired scope and its request bookkeeping reset together with the store. */
  read: Read | null
  snapshot: SkillSnapshot | null
  /// The evidence settings revision the snapshot was read at. A snapshot read at another
  /// revision was read for settings that no longer apply.
  scope: number
  error: string | null
  saving: boolean

  /// Read evidence for a language at a settings revision. Reading what the store
  /// already has, or is already reading, does nothing, which is what makes it
  /// safe for every surface that shows evidence to call it.
  load: (target: string, revision: number) => void
  /// Read again for the language and revision the store last worked on.
  reload: () => void
  /// Adopt the native save result when its selected scope is still current.
  /// A failure reaches the caller where the learner made the change.
  save: (choices: ProfileChoices) => Promise<void>
}

const initialState = {
  snapshot: null as SkillSnapshot | null,
  read: null as Read | null,
  scope: 0,
  error: null as string | null,
  saving: false,
}

/// The desired scope owns its active request and its one trailing reload.
type Read = { request: object; target: string; revision: number; pending: boolean; queued: boolean }

export const useSkillEvidenceStore = create<SkillEvidenceState>((set, get) => {
  const start = (target: string, revision: number) => {
    const request = {}
    set({ read: { request, target, revision, pending: true, queued: false }, error: null })
    void getSkillEvidence(target).then((value) => {
      const current = get().read
      if (current?.request !== request || current.queued) return
      set({ snapshot: value, scope: revision, error: null })
    }).catch((failure: unknown) => {
      if (get().read?.request === request) set({ error: String(failure) })
    }).finally(() => {
      const current = get().read
      if (current?.request !== request) return
      if (current.queued) start(target, revision)
      else set({ read: { ...current, pending: false } })
    })
  }

  return {
    ...initialState,

    load: (target, revision) => {
      const current = get().read
      if (current?.target === target && current.revision === revision && current.pending) return
      // Selecting a cached scope still abandons the other scope's request and
      // trailing reload. Its completion cannot affect this selection.
      if (get().snapshot?.target === target && get().scope === revision) {
        set({ read: { request: {}, target, revision, pending: false, queued: false }, error: null })
        return
      }
      start(target, revision)
    },

    reload: () => {
      const current = get().read
      if (!current) return
      if (current.pending) { set({ read: { ...current, queued: true } }); return }
      start(current.target, current.revision)
    },

    save: async (choices) => {
      const selected = get().read
      set({ saving: true })
      try {
        const saved = await saveSkillProfile(choices)
        if (selected && get().read === selected && selected.target === saved.target && !selected.queued) {
          // The save result supersedes an earlier read of this same scope.
          set({ snapshot: saved, scope: selected.revision, error: null,
            read: { ...selected, request: {}, pending: false, queued: false } })
        } else get().reload()
      } finally {
        set({ saving: false })
      }
    },
  }
})
