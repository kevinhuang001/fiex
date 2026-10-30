import type { ReactNode } from 'react'
import * as D from './ui/dropdown-menu'
import { Button } from './ui/button'
import { Check } from 'lucide-react'
export function Menu({
  label,
  children,
  icon,
  align = 'end',
}: {
  label: string
  children: ReactNode
  icon: ReactNode
  align?: 'start' | 'center' | 'end'
}) {
  return (
    // Non-modal: a modal Radix menu disables pointer events on the rest of the
    // page, so clicking a different toolbar menu could only dismiss the open
    // one. Non-modal menus let the second trigger receive the same click, which
    // closes the first menu and opens the target immediately.
    <D.DropdownMenu modal={false}>
      <D.DropdownMenuTrigger asChild>
        <Button variant="ghost" size="icon" title={label} aria-label={label}>
          {icon}
        </Button>
      </D.DropdownMenuTrigger>
      <D.DropdownMenuContent align={align} className="fiex-menu">
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
export const Label = D.DropdownMenuLabel
