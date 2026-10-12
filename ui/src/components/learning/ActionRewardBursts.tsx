import { reducedMotion } from '../../platform/appearance/motion'
import { useCallback, useEffect, useRef, useState, type CSSProperties } from 'react'
import { createPortal } from 'react-dom'
import { ToolbarIcon } from '../controls/ToolbarIcon'
import { supportsPopover } from '../controls/popover-support'
import { effortDimensions } from './effort-dimensions'
import { onActionReward, trackRewardActivations, type ActionReward } from '../../platform/ipc/reward-origin'

function Burst({ reward, done }: { reward: ActionReward; done: (id: string) => void }) {
  const ref = useRef<HTMLDivElement>(null)
  const popover = supportsPopover()
  useEffect(() => {
    if (popover) ref.current?.showPopover()
    const timer = setTimeout(() => done(reward.award.id), 1700)
    return () => { clearTimeout(timer) }
  }, [popover, done, reward.award.id])
  const unit = effortDimensions.find(item => item.dimension === reward.award.dimension)!
  return createPortal(<div ref={ref} popover={popover ? 'manual' : undefined} className="action-reward-burst" data-unit={unit.field} aria-hidden="true"
    style={{ left: reward.origin.x, top: reward.origin.y }}>
    <span className="action-reward-center"><ToolbarIcon name={unit.icon} size={24} /></span>
    {Array.from({ length: 6 }, (_, index) => {
      const angle = index * Math.PI / 3 - Math.PI / 2
      return <span key={index} className="action-reward-particle" style={{ '--burst-x': Math.cos(angle), '--burst-y': Math.sin(angle) } as CSSProperties}><ToolbarIcon name={unit.icon} size={14} /></span>
    })}
  </div>, document.body)
}

/** Effort icons appear at the initiating action; existing reward audio is independent. */
export function ActionRewardBursts({ enabled }: { enabled: boolean }) {
  const [bursts, setBursts] = useState<ActionReward[]>([])
  const done = useCallback((id: string) => setBursts(current => current.filter(item => item.award.id !== id)), [])
  useEffect(trackRewardActivations, [])
  useEffect(() => onActionReward(reward => {
    if (enabled && !reducedMotion()) setBursts(current => [...current.slice(-11), reward])
  }), [enabled])
  return enabled ? bursts.map(reward => <Burst key={reward.award.id} reward={reward} done={done} />) : null
}
