import { reducedMotion } from '../appearance/motion'
import { COIN_SCALES, DEFAULT_COIN_VOICE, coinNotes, validateCoinVoice, type CoinVoice } from './coin-voice'
import { reportFault } from '../diagnostics/faults'
import { visibleRewardRect } from '../../domain/rewards/reward-anchors'
import { cssToken } from '../appearance/css-token'
import { mediaError } from './media-error'
export type RewardSoundMode = 'yes' | 'no' | 'follow_tts'
export type SoundCue = { kind: 'xp'; xp: number } | { kind: 'milestone' } | { kind: 'confused' } | { kind: 'understood' } | { kind: 'pop' }
export interface Beep { frequency: number; at: number; duration: number }

export function soundEnabled(mode: RewardSoundMode, readAloud: boolean): boolean {
  return mode === 'yes' || (mode === 'follow_tts' && readAloud)
}

/** Pentatonic coin motifs: a quick pickup and a longer, higher bell note. */
export function soundPattern(cue: SoundCue, variation = 0, voice: CoinVoice = DEFAULT_COIN_VOICE): Beep[] {
  if (cue.kind === 'xp' && (!Number.isFinite(cue.xp) || cue.xp <= 0)) throw new Error('Reward sounds require positive XP.')
  if (cue.kind === 'xp' || cue.kind === 'milestone') {
    const count = cue.kind === 'milestone' ? Math.max(5, voice.notes) : cue.xp >= 20 ? Math.max(4, voice.notes) : cue.xp >= 10 ? Math.max(3, voice.notes) : voice.notes
    return coinNotes(voice, variation, count)
  }
  if (cue.kind === 'pop') return [{ frequency: 520, at: 0, duration: 0.075 }, { frequency: 1040, at: 0.055, duration: 0.12 }]
  const pitches = cue.kind === 'confused' ? [440, 370, 392] : [659, 988]
  return pitches.map((frequency, index) => ({ frequency, at: index * 0.075, duration: 0.07 }))
}

let coinVoice: CoinVoice = { ...DEFAULT_COIN_VOICE }
/** Preview tuning is session-local; application settings are not changed. */
export function configureCoinVoice(value: CoinVoice): void {
  coinVoice = validateCoinVoice(value)
}

let enabled = false
let allowed = true
let context: AudioContext | null = null
let output: GainNode | null = null
let volume = 1
let lastCoinVariation = -1
let nextStart = 0
const voices = new Set<OscillatorNode>()
const flashes = new Set<Animation>()

export function setRewardVolume(value: number): void {
  if (!Number.isFinite(value) || value < 0 || value > 1) throw new Error('Effects volume must be between 0 and 1.')
  volume = value
  if (output) output.gain.value = volume
}

export function stopRewardSounds(): void {
  for (const voice of voices) { voice.stop(); voice.disconnect() }
  voices.clear()
  for (const flash of flashes) flash.cancel()
  flashes.clear()
  nextStart = 0
}
export function configureRewardSounds(mode: RewardSoundMode, readAloud: boolean): void {
  enabled = soundEnabled(mode, readAloud)
  if (!enabled) stopRewardSounds()
}
export function setRewardPlaybackAllowed(value: boolean): void {
  allowed = value
  if (!allowed) stopRewardSounds()
}
export function unlockRewardAudio(): void {
  if (!enabled || !allowed || document.visibilityState === 'hidden') return
  if (!context) {
    try {
      context = new AudioContext()
      output = context.createGain()
      output.gain.value = volume
      output.connect(context.destination)
    } catch (error) {
      reportFault('Enabling reward sounds', mediaError(error, 'Creating reward audio context'))
      return
    }
  }
  // WebKit also pauses contexts as interrupted after native audio or app suspension.
  if (context.state !== 'running' && context.state !== 'closed') void context.resume().catch(error => reportFault('Enabling reward sounds', mediaError(error, 'Resuming reward audio context')))
}

/** No historical replay, no background queue, and no sound without a visible cause. */
export function playRewardSound(cue: SoundCue, target: HTMLElement, timing: 'queued' | 'immediate' = 'queued', plannedNotes?: Beep[]): boolean {
  if (!enabled || !allowed || volume === 0 || document.visibilityState === 'hidden' || context?.state !== 'running' || !target.isConnected) return false
  if (!visibleRewardRect(target, document.body)) return false
  const coin = cue.kind === 'xp' || cue.kind === 'milestone'
  const scaleLength = COIN_SCALES[coinVoice.scale].length
  let variation = Math.floor(Math.random() * scaleLength)
  if (variation === lastCoinVariation) variation = (variation + 1) % scaleLength
  if (coin) lastCoinVariation = variation
  const notes = plannedNotes ?? soundPattern(cue, coin ? variation : 0, coinVoice)
  const audio = context
  const start = timing === 'immediate' ? audio.currentTime : Math.max(audio.currentTime, nextStart)
  if (start - audio.currentTime > 0.45) return false
  if (timing === 'queued') nextStart = start + 0.12
  for (const note of notes) {
    const oscillator = audio.createOscillator()
    const gain = audio.createGain()
    oscillator.type = coin ? coinVoice.waveform : cue.kind === 'pop' ? 'triangle' : 'square'
    oscillator.frequency.value = note.frequency
    const at = start + note.at
    if (coin) {
      oscillator.frequency.setValueAtTime(note.frequency * 2 ** (coinVoice.bend / 12), at)
      oscillator.frequency.exponentialRampToValueAtTime(note.frequency, at + .006)
    } else if (cue.kind === 'pop') {
      oscillator.frequency.setValueAtTime(note.frequency, at)
      oscillator.frequency.exponentialRampToValueAtTime(note.frequency * 1.35, at + note.duration * .55)
    }
    gain.gain.setValueAtTime(0, at)
    gain.gain.linearRampToValueAtTime(coin ? (coinVoice.waveform === 'square' || coinVoice.waveform === 'sawtooth' ? .022 : .055) : cue.kind === 'pop' ? 0.09 : 0.025, at + (coin ? coinVoice.attackMs / 1000 : 0.004))
    gain.gain.exponentialRampToValueAtTime(0.001, at + note.duration)
    oscillator.connect(gain)
    gain.connect(output!)
    voices.add(oscillator)
    oscillator.onended = () => { voices.delete(oscillator); oscillator.disconnect(); gain.disconnect() }
    oscillator.start(at)
    oscillator.stop(at + note.duration + 0.01)
  }
  if (reducedMotion()) return true
  const flashTarget = cue.kind === 'pop' ? target.closest<HTMLElement>('.msg') ?? target : target
  const glow = cssToken('--interaction-ink')
  const flash = flashTarget.animate([
    { boxShadow: `0 0 0 2px ${glow}, 0 0 12px color-mix(in srgb, ${glow} 40%, transparent)` },
    { boxShadow: '0 0 0 0px transparent, 0 0 0px transparent' },
  ], { delay: (start - audio.currentTime) * 1000, duration: 240, easing: 'ease-out' })
  flashes.add(flash)
  flash.onfinish = () => { flashes.delete(flash) }
  return true
}
