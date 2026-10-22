import type { ReactNode } from 'react'
import * as D from './ui/dropdown-menu'
import { Button } from './ui/button'
import { Check } from 'lucide-react'
export function Menu({
  label,
  children,
  icon,
}: {
  label: string
  children: ReactNode
  icon: ReactNode
}) {
  return (
    <D.DropdownMenu>
      <D.DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" title={label} aria-label={label}>
          {icon}
        </Button>
      </D.DropdownMenuTrigger>
      <D.DropdownMenuContent align="end" className="fiex-menu">
        {children}
      </D.DropdownMenuContent>
    </D.DropdownMenu>
  )
}
export function Choice({
  children,
  checked,
  onClick,
}: {
  children: ReactNode
  checked?: boolean
  onClick: () => void
}) {
  return (
    <D.DropdownMenuItem onSelect={onClick}>
      <span className="menu-check">{checked && <Check size={14} />}</span>
      {children}
    </D.DropdownMenuItem>
  )
}
export function Action({
  children,
  onClick,
  disabled = false,
  trailing,
}: {
  children: ReactNode
  onClick: () => void
  disabled?: boolean
  trailing?: ReactNode
}) {
  const item = (
    <D.DropdownMenuItem disabled={disabled} onSelect={onClick}>
      {children}
    </D.DropdownMenuItem>
  )
  if (!trailing) return item
  return (
    <div className="menu-action-row">
      {item}
      {trailing}
    </div>
  )
}
export const Separator = D.DropdownMenuSeparator
export const Label = D.DropdownMenuLabel
