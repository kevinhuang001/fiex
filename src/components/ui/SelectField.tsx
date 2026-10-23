import { ChevronDown } from 'lucide-react'
import * as D from './dropdown-menu'

export function SelectField({
  label,
  value,
  options,
  onChange,
}: {
  label: string
  value: string
  options: { value: string; label: string }[]
  onChange: (value: string) => void
}) {
  return (
    <D.DropdownMenu>
      <D.DropdownMenuTrigger className="select-field" aria-label={label}>
        <span>{options.find((option) => option.value === value)?.label || value}</span>
        <ChevronDown size={14} />
      </D.DropdownMenuTrigger>
      <D.DropdownMenuContent className="fiex-menu select-menu" align="center">
        <D.DropdownMenuRadioGroup value={value} onValueChange={onChange}>
          {options.map((option) => (
            <D.DropdownMenuRadioItem key={option.value} value={option.value}>
              {option.label}
            </D.DropdownMenuRadioItem>
          ))}
        </D.DropdownMenuRadioGroup>
      </D.DropdownMenuContent>
    </D.DropdownMenu>
  )
}
