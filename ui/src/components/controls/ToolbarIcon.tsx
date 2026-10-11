import {
  AudioLines, Bot, BicepsFlexed, BookOpen, BookOpenText, Check, ChevronDown, ChevronLeft, CircleCheck, Cloud, Cpu,
  Database, Download, Ellipsis, ExternalLink, Globe, KeyRound, Languages,
  Keyboard, Layers, Lightbulb, LockKeyhole, Maximize2, Menu, MessageSquare, Mic,
  Minimize2, Moon, PanelTopClose, Pause, Pencil, Play, ScanSearch,
  Plus, RotateCw, Settings, SlidersHorizontal, Smile, Square,
  Star, Sun, Telescope, Trash2, TriangleAlert, UserRound, Volume2, WholeWord,
  X
} from 'lucide-react'

const LIBRARY_ICONS = {
  telescope: Telescope,
  bot: Bot,
  smile: Smile,
  practice: BicepsFlexed,
  skills: BookOpenText,
  back: ChevronLeft,
  translate: Languages,
  words: WholeWord,
  analysis: ScanSearch,
  clean: CircleCheck,
  alert: TriangleAlert,
  reload: RotateCw,
  settings: SlidersHorizontal,
  profile: UserRound,
  menu: Menu,
  more: Ellipsis,
  star: Star,
  reading: BookOpen,
  key: KeyRound,
  models: Cpu,
  globe: Globe,
  voice: Volume2,
  keyboard: Keyboard,
  update: Download,
  data: Database,
  close: X,
  cog: Settings,
  sun: Sun,
  moon: Moon,
  idea: Lightbulb,
  plus: Plus,
  check: Check,
  chat: MessageSquare,
  mic: Mic,
  pause: Pause,
  stop: Square,
  play: Play,
  trash: Trash2,
  chevron: ChevronDown,
  expand: Maximize2,
  collapse: Minimize2,
  popout: ExternalLink,
  popin: PanelTopClose,
  waveform: AudioLines,
  edit: Pencil,
  cloud: Cloud,
  layers: Layers,
  lock: LockKeyhole,
}

// Deliberate custom variants retained after icon review.
const PATHS = {
  // Analysis scan brackets with a question: coaching on the learner's message.
  coach: <><path d="M8 3H5a2 2 0 0 0-2 2v3m13-5h3a2 2 0 0 1 2 2v3M3 16v3a2 2 0 0 0 2 2h3m8 0h3a2 2 0 0 0 2-2v-3" /><path d="M9.4 9a2.6 2.6 0 1 1 3.9 2.25c-.8.46-1.3 1.05-1.3 1.95v.3M12 17h.01" /></>,
  // Written phonetic notation /ə/, distinct from playback and audio inspection.
  pronunciation: <><path d="m6 4-4 16m20-16-4 16" /><path d="M8 12h8c0-3-1.5-5-4-5-1.5 0-2.6.6-3.3 1.6M16 12c0 3-1.5 5-4 5s-4-2-4-5" /></>,
  appearance: <><circle cx="12" cy="12" r="9" /><path d="M12 3a9 9 0 0 0 0 18z" fill="currentColor" /></>,
  help: <><circle cx="12" cy="12" r="9" /><path d="M9.4 9.5a2.6 2.6 0 1 1 3.9 2.25c-.8.46-1.3 1.05-1.3 1.95v.3M12 17h.01" /></>,
  'deck-add': <><rect x="3" y="7" width="13" height="13" rx="2" /><path d="M8 4h11a2 2 0 0 1 2 2v11" /><path d="M9.5 10.5v6M6.5 13.5h6" /></>,
  'deck-added': <><rect x="3" y="7" width="13" height="13" rx="2" /><path d="M8 4h11a2 2 0 0 1 2 2v11" /><path d="m6.5 13.8 2 2 4-4.3" /></>,
  cards: <><rect x="3" y="7" width="13" height="13" rx="2" /><path d="M8 4h11a2 2 0 0 1 2 2v11" /></>,
  // Lucide's hammer with a sparkle where it strikes: fixing a message is work worth counting.
  fixes: <><path d="m15 12-9.373 9.373a1 1 0 0 1-3.001-3L12 9" /><path d="m18 15 4-4" /><path d="m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172v-.344a2 2 0 0 0-.586-1.414l-1.657-1.657A6 6 0 0 0 12.516 3H9l1.243 1.243A6 6 0 0 1 12 8.485V10l2 2h1.172a2 2 0 0 1 1.414.586L18.5 14.5" /><path d="M19.5 16.5c.35 1.75 1.25 2.65 3 3-1.75.35-2.65 1.25-3 3-.35-1.75-1.25-2.65-3-3 1.75-.35 2.65-1.25 3-3z" /></>,
  // Lucide has no confused face: the smile's face, a wavering mouth and a question mark.
  confused: <><circle cx="10.5" cy="13.5" r="8" /><path d="M8 12h.01M13 12h.01" /><path d="M6.9 17c.75-.65 1.45-.65 2.2 0s1.45.65 2.2 0 1.45-.65 2.2 0" /><path d="M17.3 3.4a2.3 2.3 0 1 1 3.1 2.2c-.6.3-.9.8-.9 1.4" /><path d="M19.5 9.6h.01" /></>,
}

export type ToolbarIconName = keyof typeof PATHS | keyof typeof LIBRARY_ICONS

export function ToolbarIcon({ name, size = 17 }: { name: ToolbarIconName; size?: number }) {
  if (name in LIBRARY_ICONS) {
    const Icon = LIBRARY_ICONS[name as keyof typeof LIBRARY_ICONS]
    return <Icon size={size} strokeWidth={1.7} aria-hidden="true" />
  }
  return <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">{PATHS[name as keyof typeof PATHS]}</svg>
}
