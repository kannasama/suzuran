import { useEffect, useRef } from 'react'

interface CheckboxProps {
  checked: boolean
  onChange: () => void
  indeterminate?: boolean
  title?: string
  className?: string
}

export function Checkbox({ checked, onChange, indeterminate, title, className }: CheckboxProps) {
  const ref = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (ref.current) ref.current.indeterminate = indeterminate ?? false
  }, [indeterminate])

  return (
    <input
      ref={ref}
      type="checkbox"
      checked={checked}
      onChange={onChange}
      title={title}
      className={[
        'cb-themed',
        'w-[13px] h-[13px] rounded-[2px]',
        'bg-bg-base border border-[#555566]',
        className ?? '',
      ].join(' ')}
    />
  )
}
